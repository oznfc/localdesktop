use jni::objects::JValue;

use super::application_context::get_application_context;

fn toast() -> Result<(), Box<dyn std::error::Error>> {
    // Get a VM for executing JNI calls
    // let ctx = ndk_context::android_context();
    // if Some(ctx) = get_application_context() {
    //     let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }?;
    //     let context = unsafe { JObject::from_raw(ctx.context().cast()) };
    //     let env = vm.attach_current_thread()?;

    //     // Create a Java string for the toast message
    //     let message = env.new_string("Hello from Rust!")?;

    //     // Get the Toast class and the makeText method ID
    //     let toast_class = env.find_class("android/widget/Toast")?;
    //     let make_text = env.get_static_method_id(
    //         toast_class,
    //         "makeText",
    //         "(Landroid/content/Context;Ljava/lang/CharSequence;I)Landroid/widget/Toast;",
    //     )?;

    //     // Call the makeText method to create a Toast object
    //     let toast = env
    //         .call_static_method(
    //             toast_class,
    //             make_text,
    //             &[
    //                 JValue::Object(&context),
    //                 JValue::Object(&message),
    //                 JValue::Int(0),
    //             ],
    //         )?
    //         .l()?;

    //     // Get the show method ID and call it to display the toast
    //     let show = env.get_method_id(toast_class, "show", "()V")?;
    //     env.call_method(toast, show, &[])?;
    // }
    Ok(())
}
