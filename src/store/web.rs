//! IndexedDB backend for the browser build.
//!
//! JSON documents are preloaded into memory at startup so `load_json` can stay synchronous;
//! media is cached lazily. Writes update the in-memory mirror immediately and drain through a
//! single ordered queue, so two saves of the same document cannot land out of order.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::rc::Rc;

use rexie::{ObjectStore, Rexie, TransactionMode};
use wasm_bindgen::JsValue;

use super::Backend;

const DOCS: &str = "docs";
const MEDIA: &str = "media";
const FILES: &str = "files";

enum WriteOp {
    Put { store: &'static str, key: String, bytes: Vec<u8> },
}

struct Inner {
    db: Rexie,
    docs: RefCell<BTreeMap<String, Vec<u8>>>,
    cache: RefCell<HashMap<String, Vec<u8>>>,
    keys: RefCell<BTreeSet<String>>,
    loading: RefCell<HashSet<String>>,
    queue: RefCell<VecDeque<WriteOp>>,
    busy: Cell<bool>,
    repaint: Rc<dyn Fn()>,
}

impl Inner {
    async fn put(&self, store: &str, key: &str, bytes: &[u8]) -> Result<(), String> {
        let tx = self.db.transaction(&[store], TransactionMode::ReadWrite).map_err(|e| e.to_string())?;
        let s = tx.store(store).map_err(|e| e.to_string())?;
        let arr = js_sys::Uint8Array::from(bytes);
        s.put(&arr.into(), Some(&JsValue::from_str(key))).await.map_err(|e| e.to_string())?;
        tx.done().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn get(&self, store: &str, key: &str) -> Result<Option<Vec<u8>>, String> {
        let tx = self.db.transaction(&[store], TransactionMode::ReadOnly).map_err(|e| e.to_string())?;
        let s = tx.store(store).map_err(|e| e.to_string())?;
        let v = s.get(JsValue::from_str(key)).await.map_err(|e| e.to_string())?;
        tx.done().await.map_err(|e| e.to_string())?;
        Ok(v.map(|v| js_sys::Uint8Array::new(&v).to_vec()))
    }

    async fn keys(&self, store: &str) -> Result<Vec<String>, String> {
        let tx = self.db.transaction(&[store], TransactionMode::ReadOnly).map_err(|e| e.to_string())?;
        let s = tx.store(store).map_err(|e| e.to_string())?;
        let keys = s.get_all_keys(None, None).await.map_err(|e| e.to_string())?;
        tx.done().await.map_err(|e| e.to_string())?;
        Ok(keys.into_iter().filter_map(|k| k.as_string()).collect())
    }

    fn enqueue(self: &Rc<Self>, op: WriteOp) {
        self.queue.borrow_mut().push_back(op);
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        let inner = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                let next = inner.queue.borrow_mut().pop_front();
                match next {
                    Some(WriteOp::Put { store, key, bytes }) => {
                        let _ = inner.put(store, &key, &bytes).await;
                        (inner.repaint)();
                    }
                    None => {
                        // No await between this check and the flag write, so a writer either
                        // sees busy=true or lands in the queue before we stop.
                        if inner.queue.borrow().is_empty() {
                            inner.busy.set(false);
                            break;
                        }
                    }
                }
            }
        });
    }
}

/// IndexedDB-backed store. `open` must be awaited before the app is constructed.
#[derive(Clone)]
pub struct IdbBackend {
    inner: Rc<Inner>,
}

impl IdbBackend {
    pub async fn open(database: &str, repaint: Rc<dyn Fn()>) -> Result<IdbBackend, String> {
        let db = Rexie::builder(database)
            .version(1)
            .add_object_store(ObjectStore::new(DOCS))
            .add_object_store(ObjectStore::new(MEDIA))
            .add_object_store(ObjectStore::new(FILES))
            .build()
            .await
            .map_err(|e| e.to_string())?;

        let inner = Rc::new(Inner {
            db,
            docs: RefCell::new(BTreeMap::new()),
            cache: RefCell::new(HashMap::new()),
            keys: RefCell::new(BTreeSet::new()),
            loading: RefCell::new(HashSet::new()),
            queue: RefCell::new(VecDeque::new()),
            busy: Cell::new(false),
            repaint,
        });

        for store in [MEDIA, FILES] {
            for key in inner.keys(store).await.unwrap_or_default() {
                inner.keys.borrow_mut().insert(key);
            }
        }
        for key in inner.keys(DOCS).await.unwrap_or_default() {
            if let Some(bytes) = inner.get(DOCS, &key).await.unwrap_or_default() {
                inner.docs.borrow_mut().insert(key, bytes);
            }
        }

        // Ask the browser to keep this origin's data when it is under pressure.
        if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
            let _ = nav.storage().persist();
        }

        Ok(IdbBackend { inner })
    }

    fn store_for(rel: &str) -> &'static str {
        if rel.starts_with("media/") { MEDIA } else { FILES }
    }
}

impl Backend for IdbBackend {
    fn read_doc(&self, name: &str) -> Option<Vec<u8>> {
        self.inner.docs.borrow().get(name).cloned()
    }

    fn write_doc(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.inner.docs.borrow_mut().insert(name.to_string(), bytes.to_vec());
        self.inner.enqueue(WriteOp::Put { store: DOCS, key: name.to_string(), bytes: bytes.to_vec() });
        Ok(())
    }

    fn exists(&self, rel: &str) -> bool {
        self.inner.keys.borrow().contains(rel)
    }

    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.inner.cache.borrow().get(rel) {
            return Some(bytes.clone());
        }
        if !self.exists(rel) {
            return None;
        }
        if !self.inner.loading.borrow_mut().insert(rel.to_string()) {
            return None; // already being fetched
        }
        let inner = self.inner.clone();
        let rel = rel.to_string();
        wasm_bindgen_futures::spawn_local(async move {
            let store = IdbBackend::store_for(&rel);
            if let Ok(Some(bytes)) = inner.get(store, &rel).await {
                inner.cache.borrow_mut().insert(rel.clone(), bytes);
            }
            inner.loading.borrow_mut().remove(&rel);
            (inner.repaint)();
        });
        None
    }

    fn write(&self, rel: &str, bytes: &[u8]) -> Result<(), String> {
        self.inner.cache.borrow_mut().insert(rel.to_string(), bytes.to_vec());
        self.inner.keys.borrow_mut().insert(rel.to_string());
        self.inner.enqueue(WriteOp::Put { store: IdbBackend::store_for(rel), key: rel.to_string(), bytes: bytes.to_vec() });
        Ok(())
    }

    fn list(&self, sub: &str) -> Vec<String> {
        let prefix = if sub.ends_with('/') { sub.to_string() } else { format!("{sub}/") };
        self.inner.keys.borrow().iter().filter(|k| k.starts_with(&prefix)).cloned().collect()
    }

    fn label(&self) -> String {
        "Browser storage (IndexedDB)".to_string()
    }
}
