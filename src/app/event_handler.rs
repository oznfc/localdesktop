use crate::app::build::WaylandBackend;
use crate::app::event_centralizer::CentralizedEvent;
use crate::utils::logging::PolarBearExpectation;
use crate::wayland::compositor::{send_frames_surface_tree, ClientState};
use smithay::backend::input::{AbsolutePositionEvent, InputEvent, KeyboardKeyEvent, TouchEvent};
use smithay::backend::renderer::element::surface::{
    render_elements_from_surface_tree, WaylandSurfaceRenderElement,
};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::utils::draw_render_elements;
use smithay::backend::renderer::{Color32F, Frame, Renderer};
use smithay::input::keyboard::FilterResult;
use smithay::input::touch::{DownEvent, MotionEvent, UpEvent};
use smithay::utils::{Rectangle, Transform, SERIAL_COUNTER};
use std::sync::Arc;

pub fn handle(event: CentralizedEvent, backend: &mut WaylandBackend) {
    match event {
        CentralizedEvent::Redraw => {
            if let Some(winit) = backend.graphic_renderer.as_mut() {
                let size = winit.window_size();
                let damage = Rectangle::from_size(size);
                {
                    let (renderer, mut framebuffer) = winit.bind().unwrap();

                    let compositor = &mut backend.compositor;

                    let elements = compositor
                        .state
                        .xdg_shell_state
                        .toplevel_surfaces()
                        .iter()
                        .flat_map(|surface| {
                            render_elements_from_surface_tree(
                                renderer,
                                surface.wl_surface(),
                                (0, 0),
                                1.0,
                                1.0,
                                Kind::Unspecified,
                            )
                        })
                        .collect::<Vec<WaylandSurfaceRenderElement<GlesRenderer>>>();

                    let mut frame = renderer
                        .render(&mut framebuffer, size, Transform::Flipped180)
                        .unwrap();
                    frame
                        .clear(Color32F::new(0.1, 0.0, 0.0, 1.0), &[damage])
                        .unwrap();
                    draw_render_elements(&mut frame, 1.0, &elements, &[damage]).unwrap();
                    // We rely on the nested compositor to do the sync for us
                    let _ = frame.finish().unwrap();

                    for surface in compositor.state.xdg_shell_state.toplevel_surfaces() {
                        send_frames_surface_tree(
                            surface.wl_surface(),
                            compositor.start_time.elapsed().as_millis() as u32,
                        );
                    }

                    if let Some(stream) = compositor
                        .listener
                        .accept()
                        .pb_expect("Failed to accept listener")
                    {
                        log::info!("Got a client: {:?}", stream);

                        let client = compositor
                            .display
                            .handle()
                            .insert_client(stream, Arc::new(ClientState::default()))
                            .unwrap();
                        compositor.clients.push(client);
                    }

                    compositor
                        .display
                        .dispatch_clients(&mut compositor.state)
                        .pb_expect("Failed to dispatch clients");
                    compositor
                        .display
                        .flush_clients()
                        .pb_expect("Failed to flush clients");
                }

                // It is important that all events on the display have been dispatched and flushed to clients before
                // swapping buffers because this operation may block.
                winit.submit(Some(&[damage])).unwrap();
            }

            // Redraw the application.
            //
            // It's preferable for applications that do not render continuously to render in
            // this event rather than in AboutToWait, since rendering in here allows
            // the program to gracefully handle redraws requested by the OS.

            // Draw.

            // Queue a RedrawRequested event.
            //
            // You only need to call this if you've determined that you need to redraw in
            // applications which do not always need to. Applications that redraw continuously
            // can render here instead.
            backend
                .graphic_renderer
                .as_ref()
                .unwrap()
                .window()
                .request_redraw();
        }
        CentralizedEvent::Input(event) => match event {
            InputEvent::Keyboard { event } => {
                let compositor = &mut backend.compositor;
                let state = &mut compositor.state;
                let serial = SERIAL_COUNTER.next_serial();
                let time = compositor.start_time.elapsed().as_millis() as u32;
                compositor.keyboard.input::<(), _>(
                    state,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |_, _, _| {
                        //
                        FilterResult::Forward
                    },
                );
            }
            InputEvent::TouchDown { event } => {
                let compositor = &mut backend.compositor;
                let state = &mut compositor.state;
                if let Some(surface) = state
                    .xdg_shell_state
                    .toplevel_surfaces()
                    .iter()
                    .next()
                    .cloned()
                {
                    compositor.keyboard.set_focus(
                        state,
                        Some(surface.wl_surface().clone()),
                        0.into(),
                    );
                    let serial = SERIAL_COUNTER.next_serial();
                    let time = compositor.start_time.elapsed().as_millis() as u32;
                    compositor.touch.down(
                        state,
                        Some((surface.wl_surface().clone(), (0f64, 0f64).into())),
                        &DownEvent {
                            slot: event.slot(),
                            location: (event.x(), event.y()).into(),
                            serial,
                            time,
                        },
                    );
                };
            }
            InputEvent::TouchUp { event } => {
                let compositor = &mut backend.compositor;
                let state = &mut compositor.state;
                if let Some(_surface) = state
                    .xdg_shell_state
                    .toplevel_surfaces()
                    .iter()
                    .next()
                    .cloned()
                {
                    let serial = SERIAL_COUNTER.next_serial();
                    let time = compositor.start_time.elapsed().as_millis() as u32;
                    compositor.touch.up(
                        state,
                        &UpEvent {
                            slot: event.slot(),
                            serial,
                            time,
                        },
                    );
                };
            }
            InputEvent::TouchMotion { event } => {
                let compositor = &mut backend.compositor;
                let state = &mut compositor.state;
                if let Some(surface) = state
                    .xdg_shell_state
                    .toplevel_surfaces()
                    .iter()
                    .next()
                    .cloned()
                {
                    let time = compositor.start_time.elapsed().as_millis() as u32;
                    compositor.touch.motion(
                        state,
                        Some((surface.wl_surface().clone(), (0f64, 0f64).into())),
                        &MotionEvent {
                            slot: event.slot(),
                            location: (event.x(), event.y()).into(),
                            time,
                        },
                    );
                };
            }
            _ => {}
        },
        _ => (),
    }
}
