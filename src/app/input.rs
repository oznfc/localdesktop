#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum KeyState {
    /// Key is released
    Released,
    /// Key is pressed
    Pressed,
}

use std::{
    collections::HashSet,
    error::Error,
    ffi::CString,
    fmt,
    hash::Hash,
    rc::Weak,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
};

use smithay::utils::{user_data::UserDataMap, IsAlive, SealedFile, Serial, SERIAL_COUNTER};
use xkbcommon_rs::{
    keycode::Keycode,
    keysym::{keysym_get_name, KeysymFlags},
    xkb_context::ContextFlags,
    xkb_state::{KeyDirection, LayoutIndex, LedIndex, StateComponent},
    Context, Keymap,
};

/// Handler trait for Seats
pub trait SeatHandler: Sized {
    /// Type used to represent the target currently holding the keyboard focus
    type KeyboardFocus: KeyboardTarget<Self> + 'static;

    /// [SeatState] getter
    fn seat_state(&mut self) -> &mut SeatState<Self>;

    /// Callback that will be notified whenever the focus of the seat changes.
    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&Self::KeyboardFocus>) {}
}
/// Delegate type for all [Seat] globals.
///
/// Events will be forwarded to an instance of the Seat global.
pub struct SeatState<D: SeatHandler> {
    pub(crate) seats: Vec<Seat<D>>,
}

impl<D: SeatHandler> fmt::Debug for SeatState<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SeatState")
            .field("seats", &self.seats)
            .finish()
    }
}

/// A Seat handle
///
/// This struct gives you access to the control of the
/// capabilities of the associated seat.
///
/// This is an handle to the inner logic, it can be cloned.
///
/// See module-level documentation for details of use.
pub struct Seat<D: SeatHandler> {
    pub(crate) arc: Arc<SeatRc<D>>,
}

impl<D: SeatHandler> fmt::Debug for Seat<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Seat").field("arc", &self.arc).finish()
    }
}

impl<D: SeatHandler> PartialEq for Seat<D> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.arc, &other.arc)
    }
}
impl<D: SeatHandler> Eq for Seat<D> {}

impl<D: SeatHandler> Hash for Seat<D> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.arc).hash(state)
    }
}

pub(crate) struct Inner<D: SeatHandler> {
    pub(crate) keyboard: Option<KeyboardHandle<D>>,
    pub(crate) global: Option<wayland_server::backend::GlobalId>,
    pub(crate) known_seats: Vec<wayland_server::Weak<wayland_server::protocol::wl_seat::WlSeat>>,
}

impl<D: SeatHandler> fmt::Debug for Inner<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner")
            .field("pointer", &self.pointer)
            .field("keyboard", &self.keyboard)
            .field("touch", &self.touch)
            .field("global", &self.global)
            .field("known_seats", &self.known_seats)
            .finish()
    }
}

pub(crate) struct SeatRc<D: SeatHandler> {
    #[allow(dead_code)]
    pub(crate) name: String,
    pub(crate) inner: Mutex<Inner<D>>,
    user_data_map: UserDataMap,
}

impl<D: SeatHandler> fmt::Debug for SeatRc<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SeatRc")
            .field("name", &self.name)
            .field("inner", &self.inner)
            .field("user_data_map", &self.user_data_map)
            .finish()
    }
}

impl<D: SeatHandler> Clone for Seat<D> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            arc: self.arc.clone(),
        }
    }
}

impl<D: SeatHandler> Default for SeatState<D> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<D: SeatHandler> SeatState<D> {
    /// Create new delegate SeatState
    pub fn new() -> Self {
        Self { seats: Vec::new() }
    }

    /// Create a new seat
    pub fn new_seat<N>(&mut self, name: N) -> Seat<D>
    where
        N: Into<String>,
    {
        let name = name.into();

        let arc = Arc::new(SeatRc {
            name,
            inner: Mutex::new(Inner {
                keyboard: None,
                global: None,
                known_seats: Vec::new(),
            }),
            user_data_map: UserDataMap::new(),
        });
        self.seats.push(Seat { arc: arc.clone() });

        Seat { arc }
    }
}

impl<D: SeatHandler + 'static> Seat<D> {
    /// Access the `UserDataMap` associated with this `Seat`
    pub fn user_data(&self) -> &UserDataMap {
        &self.arc.user_data_map
    }

    pub fn add_keyboard(
        &mut self,
        xkb_config: XkbConfig<'_>,
        repeat_delay: i32,
        repeat_rate: i32,
    ) -> Result<KeyboardHandle<D>, KeyboardError> {
        let mut inner = self.arc.inner.lock().unwrap();
        let keyboard = self::KeyboardHandle::new(xkb_config, repeat_delay, repeat_rate)?;
        if inner.keyboard.is_some() {
            // there is already a keyboard, remove it and notify the clients
            // of the change
            inner.keyboard = None;
            inner.send_all_caps();
        }
        inner.keyboard = Some(keyboard.clone());
        inner.send_all_caps();
        Ok(keyboard)
    }

    /// Access the keyboard of this seat if any
    pub fn get_keyboard(&self) -> Option<KeyboardHandle<D>> {
        self.arc.inner.lock().unwrap().keyboard.clone()
    }

    /// Remove the keyboard capability from this seat
    ///
    /// Clients will be appropriately notified.
    pub fn remove_keyboard(&mut self) {
        let mut inner = self.arc.inner.lock().unwrap();
        if inner.keyboard.is_some() {
            inner.keyboard = None;
            inner.send_all_caps();
        }
    }

    /// Gets this seat's name
    pub fn name(&self) -> &str {
        &self.arc.name
    }
}

pub(super) enum GrabStatus<G: ?Sized> {
    None,
    Active(Serial, Box<G>),
    Borrowed,
}

// `G` is not `Debug`, so we have to impl Debug manually
impl<G: ?Sized> fmt::Debug for GrabStatus<G> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrabStatus::None => f.debug_tuple("GrabStatus::None").finish(),
            GrabStatus::Active(serial, _) => {
                f.debug_tuple("GrabStatus::Active").field(&serial).finish()
            }
            GrabStatus::Borrowed => f.debug_tuple("GrabStatus::Borrowed").finish(),
        }
    }
}

/// Trait representing object that can receive keyboard interactions
pub trait KeyboardTarget<D>: IsAlive + PartialEq + Clone + fmt::Debug + Send
where
    D: SeatHandler,
{
    /// Keyboard focus of a given seat was assigned to this handler
    fn enter(&self, seat: &Seat<D>, data: &mut D, keys: Vec<KeysymHandle<'_>>, serial: Serial);
    /// The keyboard focus of a given seat left this handler
    fn leave(&self, seat: &Seat<D>, data: &mut D, serial: Serial);
    /// A key was pressed on a keyboard from a given seat
    fn key(
        &self,
        seat: &Seat<D>,
        data: &mut D,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: u32,
    );
    /// Hold modifiers were changed on a keyboard from a given seat
    fn modifiers(&self, seat: &Seat<D>, data: &mut D, modifiers: ModifiersState, serial: Serial);
    /// Keyboard focus of a given seat moved from another handler to this handler
    fn replace(
        &self,
        replaced: <D as SeatHandler>::KeyboardFocus,
        seat: &Seat<D>,
        data: &mut D,
        keys: Vec<KeysymHandle<'_>>,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        KeyboardTarget::<D>::leave(&replaced, seat, data, serial);
        KeyboardTarget::<D>::enter(self, seat, data, keys, serial);
        KeyboardTarget::<D>::modifiers(self, seat, data, modifiers, serial);
    }
}

/// Mapping of the led of a keymap
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedMapping {
    /// Index of the NUMLOCK led
    pub num: Option<LedIndex>,
    /// Index of the CAPSLOCK led
    pub caps: Option<LedIndex>,
    /// Index of the SCROLLLOCK led
    pub scroll: Option<LedIndex>,
}

/// Do not apply any flags.
pub const KEYMAP_COMPILE_NO_FLAGS: u32 = 0;
/// The current/classic XKB text format, as generated by xkbcomp -xkb.
pub const KEYMAP_FORMAT_TEXT_V1: u32 = 1;
/// Get the keymap as a string in the format from which it was created.
pub const KEYMAP_FORMAT_USE_ORIGINAL: u32 = 0xffff_ffff;
pub const KEYCODE_INVALID: u32 = 0xffff_ffff;
pub const LAYOUT_INVALID: u32 = 0xffff_ffff;
pub const LEVEL_INVALID: u32 = 0xffff_ffff;
pub const MOD_INVALID: u32 = 0xffff_ffff;
pub const LED_INVALID: u32 = 0xffff_ffff;
pub const MOD_NAME_SHIFT: &str = "Shift";
pub const MOD_NAME_CAPS: &str = "Lock";
pub const MOD_NAME_CTRL: &str = "Control";
pub const MOD_NAME_ALT: &str = "Mod1";
pub const MOD_NAME_NUM: &str = "Mod2";
pub const MOD_NAME_MOD3: &str = "Mod3";
pub const MOD_NAME_LOGO: &str = "Mod4";
pub const MOD_NAME_ISO_LEVEL3_SHIFT: &str = "Mod5";
pub const LED_NAME_CAPS: &str = "Caps Lock";
pub const LED_NAME_NUM: &str = "Num Lock";
pub const LED_NAME_SCROLL: &str = "Scroll Lock";

/// Depressed modifiers, i.e. a key is physically holding them.
pub const STATE_MODS_DEPRESSED: u32 = 1 << 0;
/// Latched modifiers, i.e. will be unset after the next non-modifier
///  key press.
pub const STATE_MODS_LATCHED: u32 = 1 << 1;
/// Locked modifiers, i.e. will be unset after the key provoking the
///  lock has been pressed again.
pub const STATE_MODS_LOCKED: u32 = 1 << 2;
/// Effective modifiers, i.e. currently active and affect key
///  processing (derived from the other state components).
///  Use this unless you explictly care how the state came about.
pub const STATE_MODS_EFFECTIVE: u32 = 1 << 3;
/// Depressed layout, i.e. a key is physically holding it.
pub const STATE_LAYOUT_DEPRESSED: u32 = 1 << 4;
/// Latched layout, i.e. will be unset after the next non-modifier
///  key press.
pub const STATE_LAYOUT_LATCHED: u32 = 1 << 5;
/// Locked layout, i.e. will be unset after the key provoking the lock
///  has been pressed again.
pub const STATE_LAYOUT_LOCKED: u32 = 1 << 6;
/// Effective layout, i.e. currently active and affects key processing
///  (derived from the other state components).
///  Use this unless you explictly care how the state came about.
pub const STATE_LAYOUT_EFFECTIVE: u32 = 1 << 7;
/// LEDs (derived from the other state components).
pub const STATE_LEDS: u32 = 1 << 8;

impl LedMapping {
    /// Get the mapping from a keymap
    pub fn from_keymap(keymap: &Keymap) -> Self {
        Self {
            num: match keymap.led_get_index(LED_NAME_NUM) {
                LED_INVALID => None,
                index => Some(index),
            },
            caps: match keymap.led_get_index(LED_NAME_CAPS) {
                LED_INVALID => None,
                index => Some(index),
            },
            scroll: match keymap.led_get_index(LED_NAME_SCROLL) {
                LED_INVALID => None,
                index => Some(index),
            },
        }
    }
}

/// Current state of the led when available
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct LedState {
    /// State of NUMLOCK led
    pub num: Option<bool>,
    /// State of CAPSLOCK led
    pub caps: Option<bool>,
    /// State of SCROLLLOCK led
    pub scroll: Option<bool>,
}

impl LedState {
    /// Update the led state from an xkb state and mapping
    ///
    /// Returns whether the led state changed
    pub fn update_with(&mut self, state: &xkbcommon_rs::State, mapping: &LedMapping) -> bool {
        let previous_state = *self;
        self.num = mapping.num.map(|idx| state.led_index_is_active(idx));
        self.caps = mapping.caps.map(|idx| state.led_index_is_active(idx));
        self.scroll = mapping.scroll.map(|idx| state.led_index_is_active(idx));
        *self != previous_state
    }

    /// Initialize the led state from an xkb state and mapping
    pub fn from_state(state: &xkbcommon_rs::State, mapping: &LedMapping) -> Self {
        let mut led_state = LedState::default();
        led_state.update_with(state, mapping);
        led_state
    }
}

/// An xkbcommon context, keymap, and state, that can be sent to another
/// thread, but should not have additional ref-counts kept on one thread.
pub struct Xkb {
    context: xkbcommon_rs::Context,
    keymap: xkbcommon_rs::Keymap,
    state: xkbcommon_rs::State,
}

impl Xkb {
    /// The xkbcommon context.
    ///
    /// # Safety
    /// A ref-count of the context should not outlive the `Xkb`
    pub unsafe fn context(&self) -> &xkbcommon_rs::Context {
        &self.context
    }

    /// The xkbcommon keymap.
    ///
    /// # Safety
    /// A ref-count of the keymap should not outlive the `Xkb`
    pub unsafe fn keymap(&self) -> &xkbcommon_rs::Keymap {
        &self.keymap
    }

    /// The xkbcommon state.
    ///
    /// # Safety
    /// A ref-count of the state should not outlive the `Xkb`
    pub unsafe fn state(&self) -> &xkbcommon_rs::State {
        &self.state
    }

    /// Get the active layout of the keyboard.
    pub fn active_layout(&self) -> Layout {
        (0..self.keymap.num_layouts())
            .find(|&idx| {
                self.state
                    .layout_index_is_active(idx, StateComponent::LAYOUT_EFFECTIVE)
            })
            .map(Layout)
            .unwrap_or_default()
    }

    /// Get the human readable name for the layout.
    pub fn layout_name(&self, layout: Layout) -> &str {
        self.keymap.layout_get_name(layout.0)
    }

    /// Iterate over layouts present in the keymap.
    pub fn layouts(&self) -> impl Iterator<Item = Layout> {
        (0..self.keymap.num_layouts()).map(Layout)
    }

    /// Returns the syms for the underlying keycode without any modifications by the current keymap
    /// state applied.
    pub fn raw_syms_for_key_in_layout(&self, keycode: Keycode, layout: Layout) -> &[KeysymFlags] {
        self.keymap.key_get_syms_by_level(keycode, layout.0, 0);
    }
}

impl fmt::Debug for Xkb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Xkb")
            .field("context", &self.context.get_raw_ptr())
            .field("keymap", &self.keymap.get_raw_ptr())
            .field("state", &self.state.get_raw_ptr())
            .finish()
    }
}

// This is OK because all parts of `xkb` will remain on the
// same thread
unsafe impl Send for Xkb {}

pub(crate) struct KbdInternal<D: SeatHandler> {
    pub(crate) focus: Option<(<D as SeatHandler>::KeyboardFocus, Serial)>,
    pending_focus: Option<<D as SeatHandler>::KeyboardFocus>,
    pub(crate) pressed_keys: HashSet<Keycode>,
    pub(crate) forwarded_pressed_keys: HashSet<Keycode>,
    pub(crate) mods_state: ModifiersState,
    xkb: Arc<Mutex<Xkb>>,
    pub(crate) repeat_rate: i32,
    pub(crate) repeat_delay: i32,
    led_mapping: LedMapping,
    pub(crate) led_state: LedState,
}

// focus_hook does not implement debug, so we have to impl Debug manually
impl<D: SeatHandler> fmt::Debug for KbdInternal<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KbdInternal")
            .field("focus", &self.focus)
            .field("pending_focus", &self.pending_focus)
            .field("pressed_keys", &self.pressed_keys)
            .field("forwarded_pressed_keys", &self.forwarded_pressed_keys)
            .field("mods_state", &self.mods_state)
            .field("xkb", &self.xkb)
            .field("repeat_rate", &self.repeat_rate)
            .field("repeat_delay", &self.repeat_delay)
            .finish()
    }
}

// This is OK because all parts of `xkb` will remain on the
// same thread
unsafe impl<D: SeatHandler> Send for KbdInternal<D> {}

impl<D: SeatHandler + 'static> KbdInternal<D> {
    fn new(
        xkb_config: XkbConfig<'_>,
        repeat_rate: i32,
        repeat_delay: i32,
    ) -> Result<KbdInternal<D>, ()> {
        // we create a new context for each keyboard because libxkbcommon is actually NOT threadsafe
        // so confining it inside the KbdInternal allows us to use Rusts mutability rules to make
        // sure nothing goes wrong.
        //
        // FIXME: This is an issue with the xkbcommon-rs crate that does not reflect this
        // non-threadsafety properly.
        let context = Context::new(ContextFlags::NO_FLAGS);
        let keymap = xkb_config.compile_keymap(&context)?;
        let state = xkbcommon_rs::State::new(&keymap);
        let led_mapping = LedMapping::from_keymap(&keymap);
        let led_state = LedState::from_state(&state, &led_mapping);
        Ok(KbdInternal {
            focus: None,
            pending_focus: None,
            pressed_keys: HashSet::new(),
            forwarded_pressed_keys: HashSet::new(),
            mods_state: ModifiersState::default(),
            xkb: Arc::new(Mutex::new(Xkb {
                context,
                keymap,
                state,
            })),
            repeat_rate,
            repeat_delay,
            led_mapping,
            led_state,
        })
    }

    // returns whether the modifiers or led state has changed
    fn key_input(&mut self, keycode: Keycode, state: KeyState) -> (bool, bool) {
        // track pressed keys as xkbcommon does not seem to expose it :(
        let direction = match state {
            KeyState::Pressed => {
                self.pressed_keys.insert(keycode);
                KeyDirection::Down
            }
            KeyState::Released => {
                self.pressed_keys.remove(&keycode);
                KeyDirection::Up
            }
        };

        // update state
        // Offset the keycode by 8, as the evdev XKB rules reflect X's
        // broken keycode system, which starts at 8.
        let mut xkb = self.xkb.lock().unwrap();
        let state_components = xkb.state.update_key(keycode, direction);
        let modifiers_changed = state_components != 0;
        if modifiers_changed {
            self.mods_state.update_with(&xkb.state);
        }
        let leds_changed = self.led_state.update_with(&xkb.state, &self.led_mapping);
        (modifiers_changed, leds_changed)
    }
}

/// Errors that can be encountered when creating a keyboard handler
#[derive(Debug)]
pub enum KeyboardError {
    /// libxkbcommon could not load the specified keymap
    BadKeymap,
    /// Smithay could not create a tempfile to share the keymap with clients
    IoError,
}

pub(crate) struct KbdRc<D: SeatHandler> {
    pub(crate) internal: Mutex<KbdInternal<D>>,
    pub(crate) keymap: Mutex<KeymapFile>,
    pub(crate) known_kbds: Mutex<Vec<Weak<wayland_server::protocol::wl_keyboard::WlKeyboard>>>,
    pub(crate) last_enter: Mutex<Option<Serial>>,
    pub(crate) active_keymap: RwLock<usize>,
}

impl<D: SeatHandler> fmt::Debug for KbdRc<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KbdRc")
            .field("internal", &self.internal)
            .field("keymap", &self.keymap)
            .field("known_kbds", &self.known_kbds)
            .field("last_enter", &self.last_enter)
            .finish()
    }
}

/// Handle to the underlying keycode to allow for different conversions
pub struct KeysymHandle<'a> {
    xkb: &'a Mutex<Xkb>,
    keycode: Keycode,
}

impl fmt::Debug for KeysymHandle<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.keycode)
    }
}

impl<'a> KeysymHandle<'a> {
    /// Get the reference to the xkb state.
    pub fn xkb(&self) -> &Mutex<Xkb> {
        self.xkb
    }

    /// Returns the sym for the underlying keycode with all modifications by the current keymap state applied.
    ///
    /// This function is similar to [`KeysymHandle::modified_syms`], but is intended for cases where the user
    /// does not want to or cannot handle multiple keysyms.
    ///
    /// If the key does not have exactly one keysym, returns [`keysyms::KEY_NoSymbol`].
    pub fn modified_sym(&self) -> KeysymFlags {
        self.xkb.lock().unwrap().state.key_get_one_sym(self.keycode)
    }

    /// Returns the syms for the underlying keycode with all modifications by the current keymap state applied.
    pub fn modified_syms(&self) -> Vec<KeysymFlags> {
        self.xkb
            .lock()
            .unwrap()
            .state
            .key_get_syms(self.keycode)
            .to_vec()
    }

    /// Returns the syms for the underlying keycode without any modifications by the current keymap state applied.
    pub fn raw_syms(&self) -> Vec<KeysymFlags> {
        let xkb = self.xkb.lock().unwrap();
        xkb.keymap
            .key_get_syms_by_level(self.keycode, xkb.state.key_get_layout(self.keycode), 0)
            .to_vec()
    }

    /// Get the raw latin keysym or fallback to current raw keysym.
    ///
    /// This method is handy to implement layout agnostic bindings. Keep in mind that
    /// it could be not-ideal to use just this function, since some layouts utilize non-standard
    /// shift levels and you should look into [`Self::modified_sym`] first.
    ///
    /// The `None` is returned when the underlying keycode doesn't produce a valid keysym.
    pub fn raw_latin_sym_or_raw_current_sym(&self) -> Option<KeysymFlags> {
        let xkb = self.xkb.lock().unwrap();
        let effective_layout = Layout(xkb.state.key_get_layout(self.keycode));

        // don't call `self.raw_syms()` to avoid a deadlock
        // and an unnecessary allocation into a Vec
        let raw_syms = xkb.keymap.key_get_syms_by_level(
            self.keycode,
            xkb.state.key_get_layout(self.keycode),
            0,
        );
        // NOTE: There's always a keysym in the current layout given that we have modified_sym.
        let base_sym = *raw_syms.first()?;

        // If the character is ascii or non-printable, return it.
        if base_sym.key_char().map(|ch| ch.is_ascii()).unwrap_or(true) {
            return Some(base_sym);
        };

        // Try to look other layouts and find the one with ascii character.
        for layout in xkb.layouts() {
            if layout == effective_layout {
                continue;
            }

            if let Some(keysym) = xkb.raw_syms_for_key_in_layout(self.keycode, layout).first() {
                // NOTE: Only check for ascii non-control characters, since control ones are
                // layout agnostic.
                if keysym
                    .key_char()
                    .map(|key| key.is_ascii() && !key.is_ascii_control())
                    .unwrap_or(false)
                {
                    return Some(*keysym);
                }
            }
        }

        Some(base_sym)
    }

    /// Returns the raw code in X keycode system (shifted by 8)
    pub fn raw_code(&'a self) -> Keycode {
        self.keycode
    }
}

/// The currently active state of the Xkb.
pub struct XkbContext<'a> {
    xkb: &'a Mutex<Xkb>,
    mods_state: &'a mut ModifiersState,
    mods_changed: &'a mut bool,
    leds_state: &'a mut LedState,
    leds_changed: &'a mut bool,
    leds_mapping: &'a LedMapping,
}

impl XkbContext<'_> {
    /// Get the reference to the xkb state.
    pub fn xkb(&self) -> &Mutex<Xkb> {
        self.xkb
    }

    /// Set layout of the keyboard to the given index.
    pub fn set_layout(&mut self, layout: Layout) {
        let mut xkb = self.xkb.lock().unwrap();

        let state = xkb.state.update_mask(
            self.mods_state.serialized.depressed,
            self.mods_state.serialized.latched,
            self.mods_state.serialized.locked,
            0,
            0,
            layout.0,
        );

        if state != 0 {
            self.mods_state.update_with(&xkb.state);
            *self.mods_changed = true;
        }

        *self.leds_changed = self.leds_state.update_with(&xkb.state, self.leds_mapping);
    }

    /// Switches layout forward cycling when it reaches the end.
    pub fn cycle_next_layout(&mut self) {
        let xkb = self.xkb.lock().unwrap();
        let next_layout = (xkb.active_layout().0 + 1) % xkb.keymap.num_layouts();
        drop(xkb);
        self.set_layout(Layout(next_layout));
    }

    /// Switches layout backward cycling when it reaches the start.
    pub fn cycle_prev_layout(&mut self) {
        let xkb = self.xkb.lock().unwrap();
        let num_layouts = xkb.keymap.num_layouts();
        let next_layout = (num_layouts + xkb.active_layout().0 - 1) % num_layouts;
        drop(xkb);
        self.set_layout(Layout(next_layout));
    }
}

impl fmt::Debug for XkbContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XkbContext")
            .field("mods_state", &self.mods_state)
            .field("mods_changed", &self.mods_changed)
            .finish()
    }
}

/// Reference to the XkbLayout in the active keymap.
///
/// The layout may become invalid after calling [`KeyboardHandle::set_xkb_config`]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout(pub LayoutIndex);

/// Result for key input filtering (see [`KeyboardHandle::input`])
#[derive(Debug)]
pub enum FilterResult<T> {
    /// Forward the given keycode to the client
    Forward,
    /// Do not forward and return value
    Intercept(T),
}

/// Data about the event that started the grab.
pub struct GrabStartData<D: SeatHandler> {
    /// The focused surface, if any, at the start of the grab.
    pub focus: Option<<D as SeatHandler>::KeyboardFocus>,
}

impl<D: SeatHandler + 'static> fmt::Debug for GrabStartData<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrabStartData")
            .field("focus", &self.focus)
            .finish()
    }
}

impl<D: SeatHandler + 'static> Clone for GrabStartData<D> {
    fn clone(&self) -> Self {
        GrabStartData {
            focus: self.focus.clone(),
        }
    }
}

/// An handle to a keyboard handler
///
/// It can be cloned and all clones manipulate the same internal state.
///
/// This handle gives you 2 main ways to interact with the keyboard handling:
///
/// - set the current focus for this keyboard: designing the surface that will receive the key inputs
///   using the [`KeyboardHandle::set_focus`] method.
/// - process key inputs from the input backend, allowing them to be caught at the compositor-level
///   or forwarded to the client. See the documentation of the [`KeyboardHandle::input`] method for
///   details.
pub struct KeyboardHandle<D: SeatHandler> {
    pub(crate) arc: Arc<KbdRc<D>>,
}

impl<D: SeatHandler> fmt::Debug for KeyboardHandle<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyboardHandle")
            .field("arc", &self.arc)
            .finish()
    }
}

impl<D: SeatHandler> Clone for KeyboardHandle<D> {
    #[inline]
    fn clone(&self) -> Self {
        KeyboardHandle {
            arc: self.arc.clone(),
        }
    }
}

impl<D: SeatHandler> ::std::cmp::PartialEq for KeyboardHandle<D> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.arc, &other.arc)
    }
}

impl<D: SeatHandler + 'static> KeyboardHandle<D> {
    /// Create a keyboard handler from a set of RMLVO rules
    pub(crate) fn new(
        xkb_config: XkbConfig<'_>,
        repeat_delay: i32,
        repeat_rate: i32,
    ) -> Result<Self, KeyboardError> {
        println!("Initializing a xkbcommon handler with keymap query");
        let internal = KbdInternal::new(xkb_config, repeat_rate, repeat_delay).map_err(|_| {
            println!("Loading keymap failed");
            Error::BadKeymap
        })?;

        let xkb = internal.xkb.lock().unwrap();

        println!("{} Loaded Keymap", xkb.keymap.layouts().next());

        let keymap_file = KeymapFile::new(&xkb.keymap);
        let active_keymap = keymap_file.id();

        drop(xkb);
        Ok(Self {
            arc: Arc::new(KbdRc {
                keymap: Mutex::new(keymap_file),
                internal: Mutex::new(internal),
                known_kbds: Mutex::new(Vec::new()),
                last_enter: Mutex::new(None),
                active_keymap: RwLock::new(active_keymap),
            }),
        })
    }

    pub(crate) fn change_keymap(
        &self,
        data: &mut D,
        focus: &Option<&mut <D as SeatHandler>::KeyboardFocus>,
        keymap: &xkbcommon_rs::Keymap,
        mods: ModifiersState,
    ) {
        let mut keymap_file = self.arc.keymap.lock().unwrap();
        keymap_file.change_keymap(keymap);

        self.send_keymap(data, focus, &keymap_file, mods);
    }

    /// Send a new wl_keyboard keymap, without updating the internal keymap.
    ///
    /// Returns `true` if the keymap changed from the previous keymap.
    pub(crate) fn send_keymap(
        &self,
        data: &mut D,
        focus: &Option<&mut <D as SeatHandler>::KeyboardFocus>,
        keymap_file: &KeymapFile,
        mods: ModifiersState,
    ) -> bool {
        use std::os::unix::io::AsFd;
        use wayland_server::{protocol::wl_keyboard::KeymapFormat, Resource};

        // Ignore request which do not change the keymap.
        let new_id = keymap_file.id();
        if new_id == *self.arc.active_keymap.read().unwrap() {
            return false;
        }
        *self.arc.active_keymap.write().unwrap() = new_id;

        // Update keymap for every wl_keyboard.
        let known_kbds = &self.arc.known_kbds;
        for kbd in &*known_kbds.lock().unwrap() {
            let Ok(kbd) = kbd.upgrade() else {
                continue;
            };

            let res = keymap_file.with_fd(kbd.version() >= 7, |fd, size| {
                kbd.keymap(KeymapFormat::XkbV1, fd.as_fd(), size as u32)
            });
            if let Err(e) = res {
                println!("Failed to send keymap to client error: {}", e);
            }
        }

        // Send updated modifiers.
        let seat = self.get_seat(data);
        if let Some(focus) = focus {
            focus.modifiers(&seat, data, mods, SERIAL_COUNTER.next_serial());
        }

        true
    }

    fn update_xkb_state(&self, data: &mut D, keymap: xkbcommon_rs::Keymap) {
        let mut internal = self.arc.internal.lock().unwrap();

        let mut state = xkbcommon_rs::State::new(&keymap);
        for key in &internal.pressed_keys {
            state.update_key(*key, KeyDirection::Down);
        }

        let led_mapping = LedMapping::from_keymap(&keymap);
        internal.led_mapping = led_mapping;
        internal.mods_state.update_with(&state);
        let leds_changed = internal.led_state.update_with(&state, &led_mapping);
        let mut xkb = internal.xkb.lock().unwrap();
        xkb.keymap = keymap.clone();
        xkb.state = state;
        drop(xkb);

        let mods = internal.mods_state;
        let focus = internal.focus.as_mut().map(|(focus, _)| focus);

        // #[cfg(not(feature = "wayland_frontend"))]
        // if let Some(focus) = focus.as_ref() {
        //     let seat = self.get_seat(data);
        //     focus.modifiers(&seat, data, mods, SERIAL_COUNTER.next_serial());
        // };

        self.change_keymap(data, &focus, &keymap, mods);

        if leds_changed {
            let led_state = internal.led_state;
            std::mem::drop(internal);
            let seat = self.get_seat(data);
            data.led_state_changed(&seat, led_state);
        }
    }

    /// Change the [`Keymap`](xkbcommon_rs::Keymap) used by the keyboard.
    ///
    /// The input is a keymap in XKB_KEYMAP_FORMAT_TEXT_V1 format.
    pub fn set_keymap_from_string(
        &self,
        data: &mut D,
        keymap: String,
    ) -> Result<(), KeyboardError> {
        // Construct the Keymap internally instead of accepting one as input
        // because libxkbcommon is not thread-safe.
        let keymap = xkbcommon_rs::Keymap::new_from_string(
            &self
                .arc
                .internal
                .lock()
                .unwrap()
                .xkb
                .lock()
                .unwrap()
                .context,
            &keymap,
            KEYMAP_FORMAT_TEXT_V1,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| {
            println!("Loading keymap from string failed");
            Error::BadKeymap
        })?;
        self.update_xkb_state(data, keymap);
        Ok(())
    }

    /// Change the [`XkbConfig`] used by the keyboard.
    pub fn set_xkb_config(
        &self,
        data: &mut D,
        xkb_config: XkbConfig<'_>,
    ) -> Result<(), KeyboardError> {
        let keymap = xkb_config
            .compile_keymap(
                &self
                    .arc
                    .internal
                    .lock()
                    .unwrap()
                    .xkb
                    .lock()
                    .unwrap()
                    .context,
            )
            .map_err(|_| {
                println!("Loading keymap from XkbConfig failed");
                Error::BadKeymap
            })?;
        self.update_xkb_state(data, keymap);
        Ok(())
    }

    /// Access the underlying Xkb state and perform mutable operations on it, like
    /// changing layouts.
    ///
    /// The changes to the state are automatically broadcasted to the focused client on exit.
    pub fn with_xkb_state<F, T>(&self, data: &mut D, mut callback: F) -> T
    where
        F: FnMut(XkbContext<'_>) -> T,
    {
        let (result, new_led_state) = {
            let internal = &mut *self.arc.internal.lock().unwrap();
            let mut mods_changed = false;
            let mut leds_changed = false;
            let state = XkbContext {
                mods_state: &mut internal.mods_state,
                xkb: &mut internal.xkb,
                mods_changed: &mut mods_changed,
                leds_state: &mut internal.led_state,
                leds_changed: &mut leds_changed,
                leds_mapping: &internal.led_mapping,
            };

            let result = callback(state);

            if mods_changed {
                if let Some((focus, _)) = internal.focus.as_mut() {
                    let seat = self.get_seat(data);
                    focus.modifiers(
                        &seat,
                        data,
                        internal.mods_state,
                        SERIAL_COUNTER.next_serial(),
                    );
                };
            }

            (result, leds_changed.then_some(internal.led_state))
        };

        if let Some(led_state) = new_led_state {
            let seat = self.get_seat(data);
            data.led_state_changed(&seat, led_state)
        }

        result
    }

    /// Remove any current grab on this keyboard, resetting it to the default behavior
    pub fn unset_grab(&self, data: &mut D) {
        let mut inner = self.arc.internal.lock().unwrap();
        if let GrabStatus::Active(_, handler) = &mut inner.grab {
            handler.unset(data);
        }
        inner.grab = GrabStatus::None;
    }

    /// Check if this keyboard is currently grabbed with this serial
    pub fn has_grab(&self, serial: Serial) -> bool {
        let guard = self.arc.internal.lock().unwrap();
        match guard.grab {
            GrabStatus::Active(s, _) => s == serial,
            _ => false,
        }
    }

    /// Check if this keyboard is currently being grabbed
    pub fn is_grabbed(&self) -> bool {
        let guard = self.arc.internal.lock().unwrap();
        !matches!(guard.grab, GrabStatus::None)
    }

    /// Returns the start data for the grab, if any.
    pub fn grab_start_data(&self) -> Option<GrabStartData<D>> {
        let guard = self.arc.internal.lock().unwrap();
        match &guard.grab {
            GrabStatus::Active(_, g) => Some(g.start_data().clone()),
            _ => None,
        }
    }

    /// Handle a keystroke
    ///
    /// All keystrokes from the input backend should be fed _in order_ to this method of the
    /// keyboard handler. It will internally track the state of the keymap.
    ///
    /// The `filter` argument is expected to be a closure which will peek at the generated input
    /// as interpreted by the keymap before it is forwarded to the focused client. If this closure
    /// returns [`FilterResult::Forward`], the input will not be sent to the client. If it returns
    /// [`FilterResult::Intercept`] a value can be passed to be returned by the whole function.
    /// This mechanism can be used to implement compositor-level key bindings for example.
    ///
    /// The module [`keysyms`](crate::input::keyboard::keysyms) exposes definitions of all possible keysyms
    /// to be compared against. This includes non-character keysyms, such as XF86 special keys.
    pub fn input<T, F>(
        &self,
        data: &mut D,
        keycode: Keycode,
        state: KeyState,
        serial: Serial,
        time: u32,
        filter: F,
    ) -> Option<T>
    where
        F: FnOnce(&mut D, &ModifiersState, KeysymHandle<'_>) -> FilterResult<T>,
    {
        let (filter_result, mods_changed) = self.input_intercept(data, keycode, state, filter);
        if let FilterResult::Intercept(val) = filter_result {
            // the filter returned `FilterResult::Intercept(T)`, we do not forward to client
            println!("Input was intercepted by filter");
            return Some(val);
        }

        self.input_forward(data, keycode, state, serial, time, mods_changed);
        None
    }

    /// Update the state of the keyboard without forwarding the event to the focused client
    ///
    /// Useful in conjunction with [`KeyboardHandle::input_forward`] in case you want
    /// to asynchronously decide if the event should be forwarded to the focused client.
    ///
    /// Prefer using [`KeyboardHandle::input`] if this decision can be done synchronously
    /// in the `filter` closure.
    pub fn input_intercept<T, F>(
        &self,
        data: &mut D,
        keycode: Keycode,
        state: KeyState,
        filter: F,
    ) -> (T, bool)
    where
        F: FnOnce(&mut D, &ModifiersState, KeysymHandle<'_>) -> T,
    {
        println!("Handling keystroke");

        let mut guard = self.arc.internal.lock().unwrap();
        let (mods_changed, leds_changed) = guard.key_input(keycode, state);
        let led_state = guard.led_state;
        let mods_state = guard.mods_state;
        let xkb = guard.xkb.clone();
        std::mem::drop(guard);

        let key_handle = KeysymHandle { xkb: &xkb, keycode };

        println!(
            "Calling input filter mods_state = {:?}, sym = {}",
            mods_state,
            keysym_get_name(key_handle.modified_sym())
        );
        let filter_result = filter(data, &mods_state, key_handle);

        if leds_changed {
            let seat = self.get_seat(data);
            data.led_state_changed(&seat, led_state);
        }

        (filter_result, mods_changed)
    }

    /// Forward a key event to the focused client
    ///
    /// Useful in conjunction with [`KeyboardHandle::input_intercept`].
    pub fn input_forward(
        &self,
        data: &mut D,
        keycode: Keycode,
        state: KeyState,
        serial: Serial,
        time: u32,
        mods_changed: bool,
    ) {
        let mut guard = self.arc.internal.lock().unwrap();
        match state {
            KeyState::Pressed => {
                guard.forwarded_pressed_keys.insert(keycode);
            }
            KeyState::Released => {
                guard.forwarded_pressed_keys.remove(&keycode);
            }
        };

        // forward to client if no keybinding is triggered
        let seat = self.get_seat(data);
        let modifiers = mods_changed.then_some(guard.mods_state);
        guard.with_grab(data, &seat, |data, handle, grab| {
            grab.input(data, handle, keycode, state, modifiers, serial, time);
        });
        if guard.focus.is_some() {
            println!("Input forwarded to client");
        } else {
            println!("No client currently focused");
        }
    }

    /// Set the current focus of this keyboard
    ///
    /// If the new focus is different from the previous one, any previous focus
    /// will be sent a [`wl_keyboard::Event::Leave`](wayland_server::protocol::wl_keyboard::Event::Leave)
    /// event, and if the new focus is not `None`,
    /// a [`wl_keyboard::Event::Enter`](wayland_server::protocol::wl_keyboard::Event::Enter) event will be sent.
    pub fn set_focus(
        &self,
        data: &mut D,
        focus: Option<<D as SeatHandler>::KeyboardFocus>,
        serial: Serial,
    ) {
        let mut guard = self.arc.internal.lock().unwrap();
        guard.pending_focus.clone_from(&focus);
        let seat = self.get_seat(data);
        guard.with_grab(data, &seat, |data, handle, grab| {
            grab.set_focus(data, handle, focus, serial);
        });
    }

    /// Return the key codes of the currently pressed keys.
    pub fn pressed_keys(&self) -> HashSet<Keycode> {
        let guard = self.arc.internal.lock().unwrap();
        guard.pressed_keys.clone()
    }

    /// Iterate over the keysyms of the currently pressed keys.
    pub fn with_pressed_keysyms<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Vec<KeysymHandle<'_>>) -> R,
        R: 'static,
    {
        let guard = self.arc.internal.lock().unwrap();
        {
            let handles = guard
                .pressed_keys
                .iter()
                .map(|keycode| KeysymHandle {
                    xkb: &guard.xkb,
                    keycode: *keycode,
                })
                .collect::<Vec<_>>();
            f(handles)
        }
    }

    /// Get the current modifiers state
    pub fn modifier_state(&self) -> ModifiersState {
        self.arc.internal.lock().unwrap().mods_state
    }

    /// Get the current led state
    pub fn led_state(&self) -> LedState {
        self.arc.internal.lock().unwrap().led_state
    }

    /// Check if keyboard has focus
    pub fn is_focused(&self) -> bool {
        self.arc.internal.lock().unwrap().focus.is_some()
    }

    /// Change the repeat info configured for this keyboard
    pub fn change_repeat_info(&self, rate: i32, delay: i32) {
        let mut guard = self.arc.internal.lock().unwrap();
        guard.repeat_delay = delay;
        guard.repeat_rate = rate;
        for kbd in &*self.arc.known_kbds.lock().unwrap() {
            let Ok(kbd) = kbd.upgrade() else {
                continue;
            };
            if kbd.version() >= 4 {
                kbd.repeat_info(rate, delay);
            }
        }
    }

    /// Access the [`Serial`] of the last `keyboard_enter` event, if that focus is still active.
    ///
    /// In other words this will return `None` again, once a `keyboard_leave` occurred.
    pub fn last_enter(&self) -> Option<Serial> {
        *self.arc.last_enter.lock().unwrap()
    }

    fn get_seat(&self, data: &mut D) -> Seat<D> {
        let seat_state = data.seat_state();
        seat_state
            .seats
            .iter()
            .find(|seat| seat.get_keyboard().map(|h| &h == self).unwrap_or(false))
            .cloned()
            .unwrap()
    }
}

impl<D> KeyboardHandle<D>
where
    D: SeatHandler,
    <D as SeatHandler>::KeyboardFocus: Clone,
{
    /// Retrieve the current keyboard focus
    pub fn current_focus(&self) -> Option<<D as SeatHandler>::KeyboardFocus> {
        self.arc
            .internal
            .lock()
            .unwrap()
            .focus
            .clone()
            .map(|(focus, _)| focus)
    }
}

/// This inner handle is accessed from inside a keyboard grab logic, and directly
/// sends event to the client
pub struct KeyboardInnerHandle<'a, D: SeatHandler> {
    inner: &'a mut KbdInternal<D>,
    seat: &'a Seat<D>,
}

impl<D: SeatHandler> fmt::Debug for KeyboardInnerHandle<'_, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyboardInnerHandle")
            .field("inner", &self.inner)
            .field("seat", &self.seat.arc.name)
            .finish()
    }
}

impl<D: SeatHandler + 'static> KeyboardInnerHandle<'_, D> {
    /// Access the current focus of this keyboard
    pub fn current_focus(&self) -> Option<&<D as SeatHandler>::KeyboardFocus> {
        self.inner.focus.as_ref().map(|f| &f.0)
    }

    /// Convert a given keycode as a [`KeysymHandle`] modified by this keyboards state
    pub fn keysym_handle(&self, keycode: Keycode) -> KeysymHandle<'_> {
        KeysymHandle {
            keycode,
            xkb: &self.inner.xkb,
        }
    }

    /// Get the current modifiers state
    pub fn modifier_state(&self) -> ModifiersState {
        self.inner.mods_state
    }

    /// Send the input to the focused keyboards
    pub fn input(
        &mut self,
        data: &mut D,
        keycode: Keycode,
        key_state: KeyState,
        modifiers: Option<ModifiersState>,
        serial: Serial,
        time: u32,
    ) {
        let (focus, _) = match self.inner.focus.as_mut() {
            Some(focus) => focus,
            None => return,
        };

        // Ensure keymap is up to date.
        if let Some(keyboard_handle) = self.seat.get_keyboard() {
            let keymap_file = keyboard_handle.arc.keymap.lock().unwrap();
            let mods = self.inner.mods_state;
            keyboard_handle.send_keymap(data, &Some(focus), &keymap_file, mods);
        }

        // key event must be sent before modifiers event for libxkbcommon
        // to process them correctly
        let key = KeysymHandle {
            xkb: &self.inner.xkb,
            keycode,
        };

        focus.key(self.seat, data, key, key_state, serial, time);
        if let Some(mods) = modifiers {
            focus.modifiers(self.seat, data, mods, serial);
        }
    }

    /// Iterate over the currently pressed keys.
    pub fn with_pressed_keysyms<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Vec<KeysymHandle<'_>>) -> R,
        R: 'static,
    {
        let handles = self
            .inner
            .pressed_keys
            .iter()
            .map(|code| self.keysym_handle(*code))
            .collect();
        f(handles)
    }

    /// Set the current focus of this keyboard
    ///
    /// If the new focus is different from the previous one, any previous focus
    /// will be sent a [`wl_keyboard::Event::Leave`](wayland_server::protocol::wl_keyboard::Event::Leave)
    /// event, and if the new focus is not `None`,
    /// a [`wl_keyboard::Event::Enter`](wayland_server::protocol::wl_keyboard::Event::Enter) event will be sent.
    pub fn set_focus(
        &mut self,
        data: &mut D,
        focus: Option<<D as SeatHandler>::KeyboardFocus>,
        serial: Serial,
    ) {
        if let Some(focus) = focus {
            let old_focus = self.inner.focus.replace((focus.clone(), serial));
            match (focus, old_focus) {
                (focus, Some((old_focus, _))) if focus == old_focus => {
                    println!("Focus unchanged");
                }
                (focus, Some((old_focus, _))) => {
                    println!("Focus set to new surface");
                    let keys = self
                        .inner
                        .forwarded_pressed_keys
                        .iter()
                        .map(|keycode| KeysymHandle {
                            xkb: &self.inner.xkb,
                            keycode: *keycode,
                        })
                        .collect();

                    focus.replace(
                        old_focus,
                        self.seat,
                        data,
                        keys,
                        self.inner.mods_state,
                        serial,
                    );
                    data.focus_changed(self.seat, Some(&focus));
                }
                (focus, None) => {
                    let keys = self
                        .inner
                        .forwarded_pressed_keys
                        .iter()
                        .map(|keycode| KeysymHandle {
                            xkb: &self.inner.xkb,
                            keycode: *keycode,
                        })
                        .collect();

                    focus.enter(self.seat, data, keys, serial);
                    focus.modifiers(self.seat, data, self.inner.mods_state, serial);
                    data.focus_changed(self.seat, Some(&focus));
                }
            }
        } else if let Some((old_focus, _)) = self.inner.focus.take() {
            println!("Focus unset");
            old_focus.leave(self.seat, data, serial);
        }
    }
}

/// Represents the current state of the keyboard modifiers
///
/// Each field of this struct represents a modifier and is `true` if this modifier is active.
///
/// For some modifiers, this means that the key is currently pressed, others are toggled
/// (like caps lock).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ModifiersState {
    /// The "control" key
    pub ctrl: bool,
    /// The "alt" key
    pub alt: bool,
    /// The "shift" key
    pub shift: bool,
    /// The "Caps lock" key
    pub caps_lock: bool,
    /// The "logo" key
    ///
    /// Also known as the "windows" key on most keyboards
    pub logo: bool,
    /// The "Num lock" key
    pub num_lock: bool,
    /// The "ISO level 3 shift" key
    ///
    /// Also known as the "AltGr" key
    pub iso_level3_shift: bool,

    /// The "ISO level 5 shift" key
    pub iso_level5_shift: bool,

    /// Serialized modifier state, as send e.g. by the wl_keyboard protocol
    pub serialized: SerializedMods,
}

impl ModifiersState {
    /// Update the modifiers state from an xkb state
    pub fn update_with(&mut self, state: &xkbcommon_rs::State) {
        self.ctrl = state.mod_name_is_active(&MOD_NAME_CTRL, STATE_MODS_EFFECTIVE);
        self.alt = state.mod_name_is_active(&MOD_NAME_ALT, STATE_MODS_EFFECTIVE);
        self.shift = state.mod_name_is_active(&MOD_NAME_SHIFT, STATE_MODS_EFFECTIVE);
        self.caps_lock = state.mod_name_is_active(&MOD_NAME_CAPS, STATE_MODS_EFFECTIVE);
        self.logo = state.mod_name_is_active(&MOD_NAME_LOGO, STATE_MODS_EFFECTIVE);
        self.num_lock = state.mod_name_is_active(&MOD_NAME_NUM, STATE_MODS_EFFECTIVE);
        self.iso_level3_shift =
            state.mod_name_is_active(&MOD_NAME_ISO_LEVEL3_SHIFT, STATE_MODS_EFFECTIVE);
        self.iso_level5_shift = state.mod_name_is_active(&MOD_NAME_MOD3, STATE_MODS_EFFECTIVE);
        self.serialized = serialize_modifiers(state);
    }
}

/// Serialized modifier state
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SerializedMods {
    /// Depressed modifiers
    pub depressed: u32,
    /// Latched modifiers
    pub latched: u32,
    /// Locked modifiers
    pub locked: u32,
    /// Effective keyboard layout
    pub layout_effective: u32,
}

fn serialize_modifiers(state: &xkbcommon_rs::State) -> SerializedMods {
    let depressed = state.serialize_mods(STATE_MODS_DEPRESSED);
    let latched = state.serialize_mods(STATE_MODS_LATCHED);
    let locked = state.serialize_mods(STATE_MODS_LOCKED);
    let layout_effective = state.serialize_layout(STATE_LAYOUT_EFFECTIVE);

    SerializedMods {
        depressed,
        latched,
        locked,
        layout_effective,
    }
}

/// Configuration for xkbcommon.
///
/// For the fields that are not set ("" or None, as set in the `Default` impl), xkbcommon will use
/// the values from the environment variables `XKB_DEFAULT_RULES`, `XKB_DEFAULT_MODEL`,
/// `XKB_DEFAULT_LAYOUT`, `XKB_DEFAULT_VARIANT` and `XKB_DEFAULT_OPTIONS`.
///
/// For details, see the [documentation at xkbcommon.org][docs].
///
/// [docs]: https://xkbcommon.org/doc/current/structxkb__rule__names.html
#[derive(Clone, Debug, Default)]
pub struct XkbConfig<'a> {
    /// The rules file to use.
    ///
    /// The rules file describes how to interpret the values of the model, layout, variant and
    /// options fields.
    pub rules: &'a str,
    /// The keyboard model by which to interpret keycodes and LEDs.
    pub model: &'a str,
    /// A comma separated list of layouts (languages) to include in the keymap.
    pub layout: &'a str,
    /// A comma separated list of variants, one per layout, which may modify or augment the
    /// respective layout in various ways.
    pub variant: &'a str,
    /// A comma separated list of options, through which the user specifies non-layout related
    /// preferences, like which key combinations are used for switching layouts, or which key is the
    /// Compose key.
    pub options: Option<String>,
}

impl XkbConfig<'_> {
    pub(crate) fn compile_keymap(
        &self,
        context: &xkbcommon_rs::Context,
    ) -> Result<xkbcommon_rs::Keymap, ()> {
        xkbcommon_rs::Keymap::new_from_names(
            context,
            self.rules,
            // self.model,
            // self.layout,
            // self.variant,
            // self.options.clone(),
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or(())
    }
}

/// Keymap ID, uniquely identifying the keymap without requiring a full content hash.
static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

/// Wraps an XKB keymap into a sealed file or stores as just a string for sending to WlKeyboard over an fd
#[derive(Debug)]
pub struct KeymapFile {
    sealed: Option<SealedFile>,
    keymap: String,
    id: usize,
}

impl KeymapFile {
    /// Turn the keymap into a string using KEYMAP_FORMAT_TEXT_V1, create a sealed file for it, and store the string
    pub fn new(keymap: &Keymap) -> Self {
        let name = c"smithay-keymap";
        let keymap = keymap.get_as_string(KEYMAP_FORMAT_TEXT_V1);
        let sealed = SealedFile::with_content(name, &CString::new(keymap.as_str()).unwrap());

        if let Err(err) = sealed.as_ref() {
            println!("Error when creating sealed keymap file: {}", err);
        }

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        Self {
            sealed: sealed.ok(),
            keymap,
            id,
        }
    }

    pub(crate) fn change_keymap(&mut self, keymap: &Keymap) {
        let keymap = keymap.get_as_string(KEYMAP_FORMAT_TEXT_V1);

        let name = c"smithay-keymap-file";
        let sealed = SealedFile::with_content(name, &CString::new(keymap.clone()).unwrap());

        if let Err(err) = sealed.as_ref() {
            println!("Error when creating sealed keymap file: {}", err);
        }

        self.id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        self.sealed = sealed.ok();
        self.keymap = keymap;
    }

    /// Send the keymap contained within to a WlKeyboard
    pub fn send(
        &self,
        keyboard: &wayland_server::protocol::wl_keyboard::WlKeyboard,
    ) -> Result<(), std::io::Error> {
        use wayland_server::{protocol::wl_keyboard::KeymapFormat, Resource};

        self.with_fd(keyboard.version() >= 7, |fd, size| {
            keyboard.keymap(KeymapFormat::XkbV1, fd, size as u32);
        })
    }

    /// Get this keymap's unique ID.
    pub(crate) fn id(&self) -> usize {
        self.id
    }
}
