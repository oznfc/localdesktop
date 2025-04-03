use jni::objects::JObject;
use jni::sys::_jobject;
use jni::JNIEnv;
use winit::platform::android::activity::AndroidApp;

// We need this function to enable fullscreen immersive mode because the below is not enough:
// android_app.set_window_flags(WindowManagerFlags::FULLSCREEN, WindowManagerFlags::empty());
// More info: https://github.com/rust-mobile/android-activity/issues/95
pub fn enable_fullscreen_immersive_mode(env: &mut JNIEnv, android_app: &AndroidApp) {
    let activity_obj = unsafe { JObject::from_raw(android_app.activity_as_ptr() as *mut _jobject) };

    // Call getWindow method
    let window = env
        .call_method(activity_obj, "getWindow", "()Landroid/view/Window;", &[])
        .expect("Failed to call getWindow")
        .l()
        .expect("Expected a Window object");

    // Call getDecorView method
    let decor_view = env
        .call_method(window, "getDecorView", "()Landroid/view/View;", &[])
        .expect("Failed to call getDecorView")
        .l()
        .expect("Expected a View object");

    // Get the View class
    let view_class = env
        .find_class("android/view/View")
        .expect("Failed to find View class");

    // Get the SYSTEM_UI_FLAG constants
    let flag_fullscreen = env
        .get_static_field(&view_class, "SYSTEM_UI_FLAG_FULLSCREEN", "I")
        .expect("Failed to get SYSTEM_UI_FLAG_FULLSCREEN")
        .i()
        .unwrap();
    let flag_hide_navigation = env
        .get_static_field(&view_class, "SYSTEM_UI_FLAG_HIDE_NAVIGATION", "I")
        .expect("Failed to get SYSTEM_UI_FLAG_HIDE_NAVIGATION")
        .i()
        .unwrap();
    let flag_immersive_sticky = env
        .get_static_field(&view_class, "SYSTEM_UI_FLAG_IMMERSIVE_STICKY", "I")
        .expect("Failed to get SYSTEM_UI_FLAG_IMMERSIVE_STICKY")
        .i()
        .unwrap();

    // Combine the flags
    let flags = flag_fullscreen | flag_hide_navigation | flag_immersive_sticky;

    // Call setSystemUiVisibility method
    env.call_method(
        decor_view,
        "setSystemUiVisibility",
        "(I)V",
        &[jni::objects::JValue::from(flags)],
    )
    .expect("Failed to call setSystemUiVisibility");
}
