use egui_winit::winit::platform::android::activity::AndroidApp;
use jni::{
    objects::{JObject, JString},
    JNIEnv, JavaVM,
};
use std::path::PathBuf;

pub struct ApplicationContext {
    pub android_app: AndroidApp,
    pub cache_dir: PathBuf,
    pub native_library_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl ApplicationContext {
    pub fn new(android_app: AndroidApp) -> Self {
        let vm = unsafe { JavaVM::from_raw(android_app.vm_as_ptr() as *mut _).unwrap() };
        let mut env = vm.attach_current_thread().unwrap();

        let activity = unsafe { JObject::from_raw(android_app.activity_as_ptr() as *mut _) };

        let cache_dir = Self::get_path(&mut env, &activity, "getCacheDir");
        let native_library_dir = Self::get_path(&mut env, &activity, "getNativeLibraryDir");
        let data_dir = Self::get_path(&mut env, &activity, "getExternalFilesDir");

        ApplicationContext {
            android_app,
            cache_dir,
            native_library_dir,
            data_dir,
        }
    }

    fn get_path(env: &mut JNIEnv, activity: &JObject, method: &str) -> PathBuf {
        let path_obj = env
            .call_method(activity, method, "()Ljava/io/File;", &[])
            .unwrap()
            .l()
            .unwrap();
        let path_str = env
            .call_method(path_obj, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .unwrap()
            .l()
            .unwrap();
        let path: String = env.get_string(&JString::from(path_str)).unwrap().into();
        PathBuf::from(path)
    }
}
