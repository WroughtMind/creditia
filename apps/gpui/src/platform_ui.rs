//! Native application chrome. Call on the GPUI foreground thread and keep one
//! adapter per window. Linux keeps its packaged launcher icon and in-app status;
//! this module reports that limitation instead of claiming native support.
use gpui_kit::{App, Global, Task, Window};

#[derive(Default)]
struct SystemMotion(Option<bool>);
impl Global for SystemMotion {}

/// Raw operating-system preference, independent of the application's switch.
/// An unavailable portal/API has no opinion; callers still honor their setting.
pub fn system_reduced_motion(cx: &App) -> Option<bool> {
    cx.try_global::<SystemMotion>().and_then(|state| state.0)
}

fn publish_system_motion(value: Option<bool>, cx: &mut App) {
    if let Some(value) = value {
        let state = cx.default_global::<SystemMotion>();
        if state.0 != Some(value) {
            state.0 = Some(value);
            cx.refresh_windows();
        }
    }
}

fn deterministic_scheduler(cx: &App) -> bool {
    cx.background_executor()
        .scheduler_executor()
        .scheduler()
        .as_test()
        .is_some()
}

/// Refresh on window activation as well as native notifications. No subprocess,
/// timer, or polling loop is used. Linux receives readings from the portal task.
pub fn refresh_system_motion(cx: &mut App) {
    if deterministic_scheduler(cx) {
        return;
    }
    #[cfg(target_os = "macos")]
    publish_system_motion(Some(macos::reduced_motion()), cx);
    #[cfg(target_os = "windows")]
    publish_system_motion(win32::reduced_motion(), cx);
}

/// Retain this task in the view. Dropping it drops the native observer guard or
/// portal stream, so closed windows leave no listener or idle wakeup behind.
/// Native callbacks only signal a bounded channel; GPUI updates stay on its
/// foreground executor rather than re-entering it from an OS callback.
pub fn watch_system_motion(cx: &mut App) -> Option<Task<()>> {
    if deterministic_scheduler(cx) {
        return None;
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let (send, receive) = async_channel::bounded(1);
        #[cfg(target_os = "macos")]
        let observer = macos::MotionObserver::new(send);
        #[cfg(target_os = "windows")]
        let observer = win32::MotionObserver::new(send);
        refresh_system_motion(cx);
        let observer = match observer {
            Ok(observer) => observer,
            Err(error) => {
                eprintln!("creditia: system motion notifications unavailable: {error}");
                return None;
            }
        };
        return Some(cx.spawn(async move |cx| {
            let _observer = observer;
            while receive.recv().await.is_ok() {
                if cx.update(refresh_system_motion).is_err() {
                    break;
                }
            }
        }));
    }
    #[cfg(target_os = "linux")]
    {
        use ashpd::desktop::settings::{ReducedMotion, Settings};
        use futures_util::StreamExt as _;
        Some(cx.spawn(async move |cx| {
            let Ok(settings) = Settings::new().await else {
                return;
            };
            // Subscribe before reading so a concurrent preference change cannot
            // disappear between the initial value and the first notification.
            let changes = settings.receive_reduced_motion_changed().await;
            if let Ok(value) = settings.reduced_motion().await {
                if cx
                    .update(|cx| {
                        publish_system_motion(
                            Some(matches!(value, ReducedMotion::ReducedMotion)),
                            cx,
                        )
                    })
                    .is_err()
                {
                    return;
                }
            }
            let Ok(mut changes) = changes else {
                return;
            };
            while let Some(value) = changes.next().await {
                if cx
                    .update(|cx| {
                        publish_system_motion(
                            Some(matches!(value, ReducedMotion::ReducedMotion)),
                            cx,
                        )
                    })
                    .is_err()
                {
                    break;
                }
            }
        }))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    None
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress {
    None,
    Indeterminate,
    Fraction(f64),
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgressState {
    None,
    Indeterminate,
    Percent(u8),
    Error,
}
impl TryFrom<Progress> for ProgressState {
    type Error = String;
    fn try_from(value: Progress) -> Result<Self, String> {
        Ok(match value {
            Progress::None => Self::None,
            Progress::Indeterminate => Self::Indeterminate,
            Progress::Error => Self::Error,
            Progress::Fraction(value) if value.is_finite() => {
                Self::Percent((value.clamp(0., 1.) * 100.).round() as u8)
            }
            Progress::Fraction(_) => return Err("Native progress must be finite".into()),
        })
    }
}

pub const fn dynamic_logo_supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}
pub const fn progress_supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}
pub fn valid_logo(id: &str) -> bool {
    matches!(id, "star" | "star-mono" | "orb" | "ring" | "color" | "mono")
}

#[derive(Default)]
pub struct PlatformUi {
    logo: Option<String>,
    progress: Option<ProgressState>,
    #[cfg(target_os = "windows")]
    windows: win32::NativeWindow,
    // OS objects and COM apartments must not migrate between executor threads.
    _foreground: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl PlatformUi {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn apply_logo(&mut self, window: &Window, id: &str) -> Result<(), String> {
        if !valid_logo(id) {
            return Err(format!("Unknown application logo: {id}"));
        }
        if !dynamic_logo_supported() {
            return Err("Dynamic launcher icons are unavailable on this platform".into());
        }
        if self.logo.as_deref() == Some(id) {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        macos::apply_logo(id)?;
        #[cfg(target_os = "windows")]
        self.windows.apply_logo(window, id)?;
        let _ = window;
        self.logo = Some(id.into());
        Ok(())
    }
    /// Repeated equal percentages are coalesced, so playback redraws never
    /// cause a native shell update at the display refresh rate.
    pub fn set_progress(&mut self, window: &Window, value: Progress) -> Result<(), String> {
        let value = ProgressState::try_from(value)?;
        if !progress_supported() {
            return Err("Native launcher progress is unavailable on this platform".into());
        }
        if self.progress == Some(value) {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        macos::set_progress(value)?;
        #[cfg(target_os = "windows")]
        self.windows.set_progress(window, value)?;
        let _ = window;
        self.progress = Some(value);
        Ok(())
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn icon_png(id: &str) -> &'static [u8] {
    match id {
        "star-mono" => include_bytes!("../../desktop/src-tauri/icons/dock/star-mono.png"),
        "orb" => include_bytes!("../../desktop/src-tauri/icons/dock/orb.png"),
        "ring" => include_bytes!("../../desktop/src-tauri/icons/dock/ring.png"),
        "color" => include_bytes!("../../desktop/src-tauri/icons/dock/color.png"),
        "mono" => include_bytes!("../../desktop/src-tauri/icons/dock/mono.png"),
        _ => include_bytes!("../../desktop/src-tauri/icons/icon.png"),
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{NSData, NSString};

    pub fn reduced_motion() -> bool {
        objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }
    pub struct MotionObserver {
        center: objc2::rc::Retained<objc2_foundation::NSNotificationCenter>,
        observer: objc2::rc::Retained<
            objc2::runtime::ProtocolObject<dyn objc2_foundation::NSObjectProtocol>,
        >,
    }
    impl MotionObserver {
        pub fn new(send: async_channel::Sender<()>) -> Result<Self, String> {
            MainThreadMarker::new().ok_or("Motion observer requires the main thread")?;
            let center = objc2_app_kit::NSWorkspace::sharedWorkspace().notificationCenter();
            let block = block2::RcBlock::new(
                move |_: std::ptr::NonNull<objc2_foundation::NSNotification>| {
                    let _ = send.try_send(());
                },
            );
            // SAFETY: The block captures only a Send+Sync channel; no AppKit or
            // GPUI value crosses threads. NSNotificationCenter retains it until
            // the matching removal in Drop. No notification object is retained.
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(
                        objc2_app_kit::NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
                    ),
                    None,
                    None,
                    &block,
                )
            };
            Ok(Self { center, observer })
        }
    }
    impl Drop for MotionObserver {
        fn drop(&mut self) {
            // SAFETY: This is exactly the observer returned by this center.
            unsafe {
                self.center.removeObserver(&self.observer);
            }
        }
    }

    pub fn apply_logo(id: &str) -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("Application icons require the main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        if id == "star" {
            // Apple's null_resettable property restores Assets.car appearances.
            unsafe { app.setApplicationIconImage(None) };
        } else {
            let data = NSData::with_bytes(icon_png(id));
            let image = NSImage::initWithData(NSImage::alloc(), &data)
                .ok_or("Cannot decode application icon")?;
            unsafe { app.setApplicationIconImage(Some(&image)) };
        }
        Ok(())
    }
    pub fn set_progress(value: ProgressState) -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("Dock progress requires the main thread")?;
        let text = match value {
            ProgressState::None => None,
            ProgressState::Indeterminate => Some(NSString::from_str("…")),
            ProgressState::Percent(percent) => Some(NSString::from_str(&format!("{percent}%"))),
            ProgressState::Error => Some(NSString::from_str("!")),
        };
        NSApplication::sharedApplication(mtm)
            .dockTile()
            .setBadgeLabel(text.as_deref());
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod win32 {
    use super::*;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::{HWND, LPARAM, RPC_E_CHANGED_MODE, WPARAM},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        UI::{
            Shell::{
                ITaskbarList3, TBPF_ERROR, TBPF_INDETERMINATE, TBPF_NOPROGRESS, TBPF_NORMAL,
                TaskbarList,
            },
            WindowsAndMessaging::{
                CreateIconFromResourceEx, DestroyIcon, HICON, ICON_BIG, ICON_SMALL, IsWindow,
                LR_DEFAULTCOLOR, SendMessageW, WM_SETICON,
            },
        },
    };

    pub fn reduced_motion() -> Option<bool> {
        use windows::{
            Win32::UI::WindowsAndMessaging::{
                SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
                SystemParametersInfoW,
            },
            core::BOOL,
        };
        let mut enabled = BOOL(0);
        // SAFETY: This action writes exactly one BOOL into the live stack slot.
        unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&mut enabled as *mut BOOL).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .ok()?;
        Some(!enabled.as_bool())
    }

    pub struct MotionObserver {
        settings: Option<windows::UI::ViewManagement::UISettings>,
        token: Option<i64>,
        runtime_initialized: bool,
        _foreground: std::marker::PhantomData<std::rc::Rc<()>>,
    }
    impl MotionObserver {
        pub fn new(send: async_channel::Sender<()>) -> Result<Self, String> {
            use windows::{
                Foundation::TypedEventHandler,
                UI::ViewManagement::{UISettings, UISettingsAnimationsEnabledChangedEventArgs},
                Win32::System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize},
            };
            // Pair every successful initialization, including an existing STA,
            // while leaving an already-established different apartment alone.
            let runtime_initialized = match unsafe { RoInitialize(RO_INIT_SINGLETHREADED) } {
                Ok(()) => true,
                Err(error) if error.code() == RPC_E_CHANGED_MODE => false,
                Err(error) => return Err(error.to_string()),
            };
            let mut observer = Self {
                settings: None,
                token: None,
                runtime_initialized,
                _foreground: std::marker::PhantomData,
            };
            let settings = UISettings::new().map_err(|error| error.to_string())?;
            let handler = TypedEventHandler::<
                UISettings,
                UISettingsAnimationsEnabledChangedEventArgs,
            >::new(move |_, _| {
                let _ = send.try_send(());
                Ok(())
            });
            let token = settings
                .AnimationsEnabledChanged(&handler)
                .map_err(|error| error.to_string())?;
            observer.settings = Some(settings);
            observer.token = Some(token);
            Ok(observer)
        }
    }
    impl Drop for MotionObserver {
        fn drop(&mut self) {
            if let (Some(settings), Some(token)) = (&self.settings, self.token.take()) {
                let _ = settings.RemoveAnimationsEnabledChanged(token);
            }
            self.settings = None;
            if self.runtime_initialized {
                unsafe {
                    windows::Win32::System::WinRT::RoUninitialize();
                }
            }
        }
    }

    #[derive(Default)]
    pub struct NativeWindow {
        hwnd: Option<HWND>,
        icon: Option<HICON>,
        taskbar: Option<ITaskbarList3>,
        com_initialized: bool,
    }
    fn handle(window: &Window) -> Result<HWND, String> {
        let handle = HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
        match handle.as_raw() {
            RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut std::ffi::c_void)),
            _ => Err("The window has no Win32 handle".into()),
        }
    }
    impl NativeWindow {
        pub fn apply_logo(&mut self, window: &Window, id: &str) -> Result<(), String> {
            let hwnd = handle(window)?;
            let png = icon_png(id);
            // RT_ICON accepts PNG resources on supported Windows versions.
            // Its documented DWORD alignment is preserved by this backing Vec.
            let words: Vec<u32> = png
                .chunks(4)
                .map(|chunk| {
                    let mut word = [0; 4];
                    word[..chunk.len()].copy_from_slice(chunk);
                    u32::from_ne_bytes(word)
                })
                .collect();
            let bytes =
                unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), png.len()) };
            let icon = unsafe {
                CreateIconFromResourceEx(bytes, true, 0x00030000, 64, 64, LR_DEFAULTCOLOR)
            }
            .map_err(|error| error.to_string())?;
            unsafe {
                SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(ICON_BIG as usize)),
                    Some(LPARAM(icon.0 as isize)),
                );
                SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(ICON_SMALL as usize)),
                    Some(LPARAM(icon.0 as isize)),
                );
            }
            self.hwnd = Some(hwnd);
            if let Some(old) = self.icon.replace(icon) {
                let _ = unsafe { DestroyIcon(old) };
            }
            Ok(())
        }
        fn taskbar(&mut self) -> Result<&ITaskbarList3, String> {
            if self.taskbar.is_none() {
                if !self.com_initialized {
                    let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
                    if result.is_ok() {
                        self.com_initialized = true;
                    } else if result != RPC_E_CHANGED_MODE {
                        return Err(format!("Cannot initialize taskbar COM: {result:?}"));
                    }
                }
                let taskbar: ITaskbarList3 =
                    unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) }
                        .map_err(|error| error.to_string())?;
                unsafe { taskbar.HrInit() }.map_err(|error| error.to_string())?;
                self.taskbar = Some(taskbar);
            }
            Ok(self.taskbar.as_ref().expect("initialized taskbar"))
        }
        pub fn set_progress(
            &mut self,
            window: &Window,
            value: ProgressState,
        ) -> Result<(), String> {
            let hwnd = handle(window)?;
            self.hwnd = Some(hwnd);
            let taskbar = self.taskbar()?;
            let state = match value {
                ProgressState::None => TBPF_NOPROGRESS,
                ProgressState::Indeterminate => TBPF_INDETERMINATE,
                ProgressState::Percent(_) => TBPF_NORMAL,
                ProgressState::Error => TBPF_ERROR,
            };
            unsafe {
                taskbar
                    .SetProgressState(hwnd, state)
                    .map_err(|error| error.to_string())?;
                if let ProgressState::Percent(percent) = value {
                    taskbar
                        .SetProgressValue(hwnd, u64::from(percent), 100)
                        .map_err(|error| error.to_string())?;
                }
                if value == ProgressState::Error {
                    taskbar
                        .SetProgressValue(hwnd, 1, 1)
                        .map_err(|error| error.to_string())?;
                }
            }
            Ok(())
        }
    }
    impl Drop for NativeWindow {
        fn drop(&mut self) {
            if let Some(hwnd) = self.hwnd {
                if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
                    if let Some(taskbar) = &self.taskbar {
                        let _ = unsafe { taskbar.SetProgressState(hwnd, TBPF_NOPROGRESS) };
                    }
                    if self.icon.is_some() {
                        unsafe {
                            SendMessageW(
                                hwnd,
                                WM_SETICON,
                                Some(WPARAM(ICON_BIG as usize)),
                                Some(LPARAM(0)),
                            );
                            SendMessageW(
                                hwnd,
                                WM_SETICON,
                                Some(WPARAM(ICON_SMALL as usize)),
                                Some(LPARAM(0)),
                            );
                        }
                    }
                }
            }
            if let Some(icon) = self.icon.take() {
                let _ = unsafe { DestroyIcon(icon) };
            }
            self.taskbar.take();
            if self.com_initialized {
                unsafe { CoUninitialize() };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "visual-test")]
    #[gpui_kit::test]
    fn raw_system_motion_survives_application_overrides_and_tests_never_watch_the_os(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|cx| {
            assert_eq!(system_reduced_motion(cx), None);
            publish_system_motion(Some(true), cx);
            cx.set_reduce_motion(false);
            assert_eq!(system_reduced_motion(cx), Some(true));
            publish_system_motion(None, cx);
            assert_eq!(system_reduced_motion(cx), Some(true));
            publish_system_motion(Some(false), cx);
            cx.set_reduce_motion(true);
            assert_eq!(system_reduced_motion(cx), Some(false));
            assert!(watch_system_motion(cx).is_none());
            refresh_system_motion(cx);
            assert_eq!(system_reduced_motion(cx), Some(false));
        });
        cx.run_until_parked();
    }
    #[test]
    fn native_progress_is_bounded_and_coalesces_subpercent_changes() {
        assert_eq!(
            ProgressState::try_from(Progress::Fraction(-3.)).unwrap(),
            ProgressState::Percent(0)
        );
        assert_eq!(
            ProgressState::try_from(Progress::Fraction(2.)).unwrap(),
            ProgressState::Percent(100)
        );
        assert_eq!(
            ProgressState::try_from(Progress::Fraction(0.123)).unwrap(),
            ProgressState::try_from(Progress::Fraction(0.124)).unwrap()
        );
        assert!(ProgressState::try_from(Progress::Fraction(f64::NAN)).is_err());
        assert!(ProgressState::try_from(Progress::Fraction(f64::INFINITY)).is_err());
    }
    #[test]
    fn exactly_the_existing_six_logos_are_recognized() {
        for id in ["star", "star-mono", "orb", "ring", "color", "mono"] {
            assert!(valid_logo(id));
        }
        assert!(!valid_logo("../../custom.png"));
        assert!(!valid_logo(""));
    }
}
