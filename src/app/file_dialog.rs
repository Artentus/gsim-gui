#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::path::{Path, PathBuf};

    pub struct FileDialog {
        open_file: Option<(PathBuf, Vec<u8>)>,
    }

    impl FileDialog {
        #[inline]
        pub fn new() -> Option<Self> {
            Some(Self { open_file: None })
        }

        pub fn open(&mut self) {
            self.open_file = rfd::FileDialog::new().pick_file().and_then(|path| {
                let data = std::fs::read(&path).ok()?;
                Some((path, data))
            });
        }

        #[inline]
        pub fn get(&mut self) -> Option<(PathBuf, Vec<u8>)> {
            self.open_file.take()
        }

        pub fn save(
            &self,
            file_name: Option<&Path>,
            data: &[u8],
        ) -> std::io::Result<Option<PathBuf>> {
            let mut dialog = rfd::FileDialog::new();
            if let Some(file_name) = file_name {
                dialog = dialog.set_file_name(file_name.to_str().expect("invalid path"));
            }

            if let Some(path) = dialog.save_file() {
                std::fs::write(&path, data)?;
                Ok(Some(path))
            } else {
                Ok(None)
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::FileDialog;

#[cfg(target_arch = "wasm32")]
mod web {
    use js_sys::{Array, ArrayBuffer, Uint8Array};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{window, File, FileReader, HtmlAnchorElement, HtmlInputElement, Url};

    pub struct FileDialog {
        tx: std::sync::mpsc::Sender<Vec<u8>>,
        rx: std::sync::mpsc::Receiver<Vec<u8>>,
        open_input: HtmlInputElement,
        open_closure: Option<Closure<dyn FnMut()>>,
        save_url: Option<String>,
    }

    impl Drop for FileDialog {
        fn drop(&mut self) {
            self.open_input.remove();
            if let Some(open_closure) = self.open_closure.take() {
                open_closure.forget();
            }
            if let Some(save_url) = self.save_url.take() {
                let _ = Url::revoke_object_url(&save_url);
            }
        }
    }

    impl FileDialog {
        pub fn new() -> Option<Self> {
            let (tx, rx) = std::sync::mpsc::channel();

            let document = window()?.document()?;
            let body = document.body()?;
            let open_input = document
                .create_element("input")
                .ok()?
                .dyn_into::<HtmlInputElement>()
                .ok()?;
            open_input.set_attribute("type", "file").ok()?;
            open_input.style().set_property("display", "none").ok()?;
            body.append_child(&open_input).ok()?;

            Some(Self {
                rx,
                tx,
                open_input,
                open_closure: None,
                save_url: None,
            })
        }

        pub fn open(&mut self) {
            if let Some(open_closure) = &self.open_closure {
                self.open_input
                    .remove_event_listener_with_callback(
                        "change",
                        open_closure.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                if let Some(open_closure) = self.open_closure.take() {
                    open_closure.forget();
                }
            }

            let tx = self.tx.clone();
            let open_input_clone = self.open_input.clone();

            let open_closure = Closure::once(move || {
                if let Some(file) = open_input_clone.files().and_then(|files| files.get(0)) {
                    let reader = FileReader::new().unwrap();
                    let reader_clone = reader.clone();
                    let onload_closure = Closure::once(Box::new(move || {
                        let array_buffer = reader_clone
                            .result()
                            .unwrap()
                            .dyn_into::<ArrayBuffer>()
                            .unwrap();
                        let buffer = Uint8Array::new(&array_buffer).to_vec();
                        tx.send(buffer).ok();
                    }));

                    reader.set_onload(Some(onload_closure.as_ref().unchecked_ref()));
                    reader.read_as_array_buffer(&file).unwrap();
                    onload_closure.forget();
                }
            });

            self.open_input
                .add_event_listener_with_callback("change", open_closure.as_ref().unchecked_ref())
                .unwrap();
            self.open_closure = Some(open_closure);
            self.open_input.click();
        }

        pub fn get(&self) -> Option<Vec<u8>> {
            self.rx.try_recv().ok()
        }

        pub fn save(&mut self, name: &str, data: &[u8]) {
            if let Some(save_url) = self.save_url.take() {
                let _ = Url::revoke_object_url(&save_url);
            }

            let name = format!("{name}.json");

            let array = Uint8Array::from(data);
            let blob_parts = Array::new();
            blob_parts.push(&array.buffer());

            let file = File::new_with_blob_sequence_and_options(
                &blob_parts.into(),
                &name,
                web_sys::FilePropertyBag::new().type_("application/octet-stream"),
            )
            .unwrap();

            let url = Url::create_object_url_with_blob(&file).unwrap();

            let document = window().unwrap().document().unwrap();
            let temp = document
                .create_element("a")
                .unwrap()
                .unchecked_into::<HtmlAnchorElement>();
            temp.set_href(&url);
            temp.set_download(&name);
            temp.click();
            temp.remove();

            self.save_url = Some(url);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::FileDialog;
