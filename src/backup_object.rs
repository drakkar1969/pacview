use std::cell::{OnceCell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: BackupObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::BackupObject)]
    pub struct BackupObject {
        #[property(get, set)]
        filename: RefCell<String>,
        #[property(get, set)]
        hash: RefCell<String>,
        #[property(get, set, nullable)]
        package: RefCell<Option<String>>,

        #[property(get = Self::status_icon)]
        _status_icon: RefCell<String>,
        #[property(get = Self::status_text)]
        _status_text: RefCell<String>,

        file_hash: OnceCell<Result<String, alpm::ChecksumError>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupObject {
        const NAME: &'static str = "BackupObject";
        type Type = super::BackupObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for BackupObject {}

    impl BackupObject {
        //-----------------------------------
        // Custom property getters
        //-----------------------------------
        fn file_hash(&self) -> &Result<String, alpm::ChecksumError> {
            self.file_hash.get_or_init(|| alpm::compute_md5sum(self.filename.borrow().as_str()))
        }

        fn status_icon(&self) -> String {
            if let Ok(file_hash) = self.file_hash() {
                if file_hash == &*self.hash.borrow() {
                    "backup-unmodified-symbolic"
                } else {
                    "backup-modified-symbolic"
                }
            } else {
                "backup-error-symbolic"
            }
            .to_string()
        }
        
        fn status_text(&self) -> String {
            if let Ok(file_hash) = self.file_hash() {
                if file_hash == &*self.hash.borrow() {
                    "unmodified"
                } else {
                    "modified"
                }
            } else {
                "read error"
            }
            .to_string()
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: BackupObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct BackupObject(ObjectSubclass<imp::BackupObject>);
}

impl BackupObject {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(filename: &str, hash: &str, package: Option<&str>) -> Self {
        // Build BackupObject
        glib::Object::builder()
            .property("filename", filename)
            .property("hash", hash)
            .property("package", package)
            .build()
    }
}
