use dashmap::DashMap;
use jni::{JNIEnv, errors::Result, objects::GlobalRef};
use once_cell::sync::OnceCell;

static CLASSCACHE: OnceCell<DashMap<String, GlobalRef>> = OnceCell::new();

pub fn find_add_class(env: &JNIEnv, classname: &str) -> Result<()> {
    let cache = CLASSCACHE.get_or_init(|| DashMap::new());
    cache.insert(
        classname.to_owned(),
        env.new_global_ref(env.find_class(classname).unwrap())
            .unwrap(),
    );
    Ok(())
}

pub fn get_class(classname: &str) -> Option<GlobalRef> {
    let cache = CLASSCACHE.get_or_init(|| DashMap::new());
    cache
        .get(classname)
        .and_then(|pair| Some(pair.value().clone()))
}
