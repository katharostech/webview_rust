use std::os::raw::*;
use std::sync::{Arc, Weak};

use crate::Error;

pub enum Window {}

#[repr(i32)]
#[derive(Debug)]
pub enum SizeHint {
    NONE = 0,
    MIN = 1,
    MAX = 2,
    FIXED = 3,
}

impl Default for SizeHint {
    fn default() -> Self {
        SizeHint::NONE
    }
}

#[cfg(not(feature = "chrome-backend"))]
pub use webview_backend::*;

#[cfg(not(feature = "chrome-backend"))]
mod webview_backend {
    use super::*;

    use std::ffi::{CStr, CString};
    use std::mem;
    use std::ptr::null_mut;

    use webview_official_sys as sys;

    #[derive(Clone)]
    pub struct Webview<'a> {
        inner: Arc<sys::webview_t>,
        url: &'a str,
    }

    impl<'a> Drop for Webview<'a> {
        fn drop(&mut self) {
            if Arc::strong_count(&self.inner) == 0 {
                unsafe {
                    sys::webview_terminate(*self.inner);
                    sys::webview_destroy(*self.inner);
                }
            }
        }
    }

    impl<'a> Webview<'a> {
        pub fn create(debug: bool, window: Option<&mut Window>) -> Webview {
            if let Some(w) = window {
                Webview {
                    inner: Arc::new(unsafe {
                        sys::webview_create(debug as c_int, w as *mut Window as *mut _)
                    }),
                    url: "",
                }
            } else {
                Webview {
                    inner: Arc::new(unsafe { sys::webview_create(debug as c_int, null_mut()) }),
                    url: "",
                }
            }
        }

        pub fn run(&mut self) {
            let c_url = CString::new(self.url).expect("No null bytes in parameter url");
            unsafe { sys::webview_navigate(*self.inner, c_url.as_ptr()) }
            unsafe { sys::webview_run(*self.inner) }
        }

        pub fn terminate(&mut self) {
            unsafe { sys::webview_terminate(*self.inner) }
        }

        pub fn as_mut(&mut self) -> WebviewMut {
            WebviewMut(Arc::downgrade(&self.inner))
        }

        // TODO Window instance
        pub fn set_title(&mut self, title: &str) {
            let c_title = CString::new(title).expect("No null bytes in parameter title");
            unsafe { sys::webview_set_title(*self.inner, c_title.as_ptr()) }
        }

        pub fn set_size(&mut self, width: i32, height: i32, hints: SizeHint) {
            unsafe { sys::webview_set_size(*self.inner, width, height, hints as i32) }
        }

        pub fn get_window(&self) -> *mut Window {
            unsafe { sys::webview_get_window(*self.inner) as *mut Window }
        }

        pub fn navigate(&mut self, url: &'a str) {
            self.url = url;
        }

        pub fn init(&mut self, js: &str) {
            let c_js = CString::new(js).expect("No null bytes in parameter js");
            unsafe { sys::webview_init(*self.inner, c_js.as_ptr()) }
        }

        pub fn eval(&mut self, js: &str) {
            let c_js = CString::new(js).expect("No null bytes in parameter js");
            unsafe { sys::webview_eval(*self.inner, c_js.as_ptr()) }
        }

        pub fn dispatch<F>(&mut self, f: F)
        where
            F: FnOnce(&mut Webview) + Send + 'static,
        {
            let closure = Box::into_raw(Box::new(f));
            extern "C" fn callback<F>(webview: sys::webview_t, arg: *mut c_void)
            where
                F: FnOnce(&mut Webview) + Send + 'static,
            {
                let mut webview = Webview {
                    inner: Arc::new(webview),
                    url: "",
                };
                let closure: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
                (*closure)(&mut webview);
            }
            unsafe { sys::webview_dispatch(*self.inner, Some(callback::<F>), closure as *mut _) }
        }

        pub fn bind<F>(&mut self, name: &str, f: F)
        where
            F: FnMut(&str, &str),
        {
            let c_name = CString::new(name).expect("No null bytes in parameter name");
            let closure = Box::into_raw(Box::new(f));
            extern "C" fn callback<F>(seq: *const c_char, req: *const c_char, arg: *mut c_void)
            where
                F: FnMut(&str, &str),
            {
                let seq = unsafe {
                    CStr::from_ptr(seq)
                        .to_str()
                        .expect("No null bytes in parameter seq")
                };
                let req = unsafe {
                    CStr::from_ptr(req)
                        .to_str()
                        .expect("No null bytes in parameter req")
                };
                let mut f: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
                (*f)(seq, req);
                mem::forget(f);
            }
            unsafe {
                sys::webview_bind(
                    *self.inner,
                    c_name.as_ptr(),
                    Some(callback::<F>),
                    closure as *mut _,
                )
            }
        }

        pub fn r#return(&self, seq: &str, status: c_int, result: &str) {
            let c_seq = CString::new(seq).expect("No null bytes in parameter seq");
            let c_result = CString::new(result).expect("No null bytes in parameter result");
            unsafe { sys::webview_return(*self.inner, c_seq.as_ptr(), status, c_result.as_ptr()) }
        }
    }

    #[derive(Clone)]
    pub struct WebviewMut(Weak<sys::webview_t>);

    unsafe impl Send for WebviewMut {}
    unsafe impl Sync for WebviewMut {}

    impl WebviewMut {
        pub fn terminate(&mut self) -> Result<(), Error> {
            let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
            unsafe { sys::webview_terminate(*webview) }
            Ok(())
        }

        pub fn get_window(&self) -> Result<*mut Window, Error> {
            let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
            Ok(unsafe { sys::webview_get_window(*webview) as *mut Window })
        }

        pub fn dispatch<F>(&mut self, f: F) -> Result<(), Error>
        where
            F: FnOnce(&mut Webview) + Send + 'static,
        {
            let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
            let closure = Box::into_raw(Box::new(f));
            extern "C" fn callback<F>(webview: sys::webview_t, arg: *mut c_void)
            where
                F: FnOnce(&mut Webview) + Send + 'static,
            {
                let mut webview = Webview {
                    inner: Arc::new(webview),
                    url: "",
                };
                let closure: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
                (*closure)(&mut webview);
            }
            unsafe { sys::webview_dispatch(*webview, Some(callback::<F>), closure as *mut _) }
            Ok(())
        }

        pub fn bind<F>(&mut self, name: &str, f: F) -> Result<(), Error>
        where
            F: FnMut(&str, &str) + 'static,
        {
            let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
            let c_name = CString::new(name).expect("No null bytes in parameter name");
            let closure = Box::into_raw(Box::new(f));
            extern "C" fn callback<F>(seq: *const c_char, req: *const c_char, arg: *mut c_void)
            where
                F: FnMut(&str, &str) + 'static,
            {
                let seq = unsafe {
                    CStr::from_ptr(seq)
                        .to_str()
                        .expect("No null bytes in parameter seq")
                };
                let req = unsafe {
                    CStr::from_ptr(req)
                        .to_str()
                        .expect("No null bytes in parameter req")
                };
                let mut f: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
                (*f)(seq, req);
                mem::forget(f);
            }
            unsafe {
                sys::webview_bind(
                    *webview,
                    c_name.as_ptr(),
                    Some(callback::<F>),
                    closure as *mut _,
                )
            }
            Ok(())
        }

        pub fn r#return(&self, seq: &str, status: c_int, result: &str) -> Result<(), Error> {
            let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
            let c_seq = CString::new(seq).expect("No null bytes in parameter seq");
            let c_result = CString::new(result).expect("No null bytes in parameter result");
            unsafe { sys::webview_return(*webview, c_seq.as_ptr(), status, c_result.as_ptr()) }
            Ok(())
        }
    }
}

#[cfg(feature = "chrome-backend")]
pub use chrome_backend::*;

#[cfg(feature = "chrome-backend")]
mod chrome_backend {
    use super::*;

    use headless_chrome::{
        protocol::{browser::Bounds, Event},
        Browser, LaunchOptionsBuilder, Tab,
    };
    use std::sync::{
        mpsc::{channel, Receiver, Sender},
        RwLock,
    };

    struct WebviewData {
        _browser: Option<Browser>,
        tab: Option<Arc<Tab>>,
        shutdown_sender: Sender<()>,
        shutdown_receiver: Arc<Receiver<()>>,
    }

    #[derive(Clone)]
    pub struct Webview<'a> {
        data: Arc<RwLock<WebviewData>>,
        url: &'a str,
    }

    impl<'a> Webview<'a> {
        pub fn create(_debug: bool, window: Option<&mut Window>) -> Webview {
            if window.is_some() {
                unimplemented!("Custom windows not supported for the chrome backend.");
            }

            let (shutdown_sender, shutdown_receiver) = channel();

            Webview {
                url: "",
                data: Arc::new(RwLock::new(WebviewData {
                    _browser: None,
                    tab: None,
                    shutdown_sender,
                    shutdown_receiver: Arc::new(shutdown_receiver),
                })),
            }
        }

        pub fn run(&mut self) {
            // TODO: We can't raise errors during a webview run??
            // For now just panic I guess 😕
            let options = LaunchOptionsBuilder::default()
                .headless(false)
                .default_args(false)
                .extra_args(vec![String::from("--no-sandbox")])
                // Start off with a completely blank html page
                .app_url(Some("data:text/html,%3Chtml%3E%3C%2Fhtml%3E".into()))
                .build()
                .expect("Couldn't find appropriate Chrome binary.");

            let browser = Browser::new(options).expect("Unable to find chrome");
            let tab = browser
                .wait_for_initial_tab()
                .expect("Error getting initial chrome tab");

            tab.add_event_listener(Arc::new(move |chrome_tab_event: &Event| {
                dbg!(chrome_tab_event);
            }))
            .expect("Could not add event listener");

            tab.navigate_to(self.url)
                .expect("Could not navigate to app");

            self.data.write().unwrap()._browser = Some(browser);
            self.data.write().unwrap().tab = Some(tab);

            let shutdown_sender = self.data.read().unwrap().shutdown_sender.clone();
            let _tab = self.data.read().unwrap().tab.clone().unwrap();

            // Spawn a thread that will check every second to make sure that
            // chrome is still running and to send the shutdown signal if it is
            // not
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                // If there is an error doing a simple JS eval then assume that
                // the connection to chrome has been lost and we need to
                // shutdown
                if let Err(e) = _tab.evaluate("window", false) {
                    shutdown_sender.send(()).ok();
                }
            });

            self.data.read().unwrap().shutdown_receiver.recv().ok();
        }

        pub fn terminate(&mut self) {
            self.data.read().unwrap().shutdown_sender.send(()).ok();
        }

        pub fn as_mut(&mut self) -> WebviewMut {
            WebviewMut(Arc::downgrade(&self.data))
        }

        pub fn set_title(&mut self, title: &str) {
            if let Some(tab) = self.data.read().unwrap().tab.as_ref() {
                tab.evaluate(
                    &format!(
                        "document.title = {}",
                        serde_json::to_string(&serde_json::Value::String(title.into()))
                            .expect("Could not serialize plain JSON string!")
                    ),
                    false,
                )
                .expect("Exec JS to set title");
            }
        }

        pub fn set_size(&mut self, width: i32, height: i32, _hints: SizeHint) {
            if let Some(tab) = self.data.read().unwrap().tab.as_ref() {
                use std::convert::TryInto;
                tab.set_bounds(Bounds::Normal {
                    height: Some(height.try_into().expect("Could not convert size to u32")),
                    width: Some(width.try_into().expect("Could not convert size to u32")),
                    top: None,
                    left: None,
                })
                .expect("Could not set window bounds");
            }
        }

        pub fn get_window(&self) -> *mut Window {
            unimplemented!("Getting window is not supported for the chrome backend");
        }

        pub fn navigate(&mut self, url: &'a str) {
            self.url = url;
            if let Some(tab) = self.data.read().unwrap().tab.as_ref() {
                tab.navigate_to(&url)
                    .expect("Could not navigate browser window");
            }
        }

        pub fn init(&mut self, _js: &str) {
            eprintln!("WARN: Webview `init` not implemented for chrome backend yet.");
        }

        pub fn eval(&mut self, js: &str) {
            if let Some(tab) = self.data.read().unwrap().tab.as_ref() {
                tab.evaluate(js, true).expect("Could not eval JS");
            }
        }

        pub fn dispatch<F>(&mut self, f: F)
        where
            F: FnOnce(&mut Webview) + Send + 'static,
        {
            eprintln!("WARN: Using `dispatch` for chrome backend is probably not useful");
            f(self)
        }

        pub fn bind<F>(&mut self, _name: &str, _f: F)
        where
            F: FnMut(&str, &str),
        {
            eprintln!("WARN: Webview `bind` is not implemented for chrome backend yet!")
        }

        pub fn r#return(&self, _seq: &str, _status: c_int, _result: &str) {
            eprintln!("WARN: Webview `return` is not implemented for chrome backend yet!");
        }
    }

    #[derive(Clone)]
    pub struct WebviewMut(Weak<RwLock<WebviewData>>);

    unsafe impl Send for WebviewMut {}
    unsafe impl Sync for WebviewMut {}

    impl WebviewMut {
        pub fn terminate(&mut self) -> Result<(), Error> {
            self.0
                .upgrade()
                .ok_or(Error::WebviewNull)?
                .read()
                .unwrap()
                .shutdown_sender
                .send(())
                .ok();

            Ok(())
        }

        pub fn get_window(&self) -> Result<*mut Window, Error> {
            unimplemented!("Cannot get window when using Chrome backend");
        }

        pub fn dispatch<F>(&mut self, _f: F) -> Result<(), Error>
        where
            F: FnOnce(&mut Webview) + Send + 'static,
        {
            unimplemented!("Cannot dispatch in a WebviewMut when using Chrome backend");
        }

        pub fn bind<F>(&mut self, _name: &str, _f: F) -> Result<(), Error>
        where
            F: FnMut(&str, &str) + 'static,
        {
            eprintln!("WARN: Webview `return` not implemented for Chrome backend yet.");
            Ok(())
        }

        pub fn r#return(&self, _seq: &str, _status: c_int, _result: &str) -> Result<(), Error> {
            eprintln!("WARN: Webview `return` not implemented for Chrome backend yet.");
            Ok(())
        }
    }
}
