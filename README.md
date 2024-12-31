# New Polar Bear

Polar Bear helps you run a desktop Linux environment on your Android device.

> This is a Rust rewrite of the original [Polar Bear](https://github.com/polar-bear-app/polar-bear-app) project, which was written in Kotlin and C++. The aim of this rewrite is to make it more stable, portable, and able to do the development work on Android.

## Getting Started

### Build an APK

```bash
x build --arch arm64 --platform android
```

### Developing

It is recommended to use Visual Studio Code with the [Rust Analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extension. Use the debugger to run the app on your host device to ease the development.

Additionally, you can use the following command to run the app on your virtual devices or attached physical devices:

```bash
x devices
x run --device <DEVICE>
```

> By doing so, the production environment will be drastically different from the development environment, but it is more convenient to develop. Thorough testing is required to ensure that the production environment is stable.