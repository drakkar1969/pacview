use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// ENUM: BackupStatus
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "BackupStatus")]
pub enum BackupStatus {
    Unmodified = 0,
    Modified = 1,
    Error = 2,
}

impl Default for BackupStatus {
    fn default() -> Self {
        BackupStatus::Unmodified
    }
}

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
        status_icon: RefCell<String>,
        #[property(get, set)]
        status_text: RefCell<String>,
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
    pub fn new(filename: &str, status: BackupStatus) -> Self {
        let status_icon = match status {
            BackupStatus::Unmodified => "backup-unmodified-symbolic",
            BackupStatus::Modified => "backup-modified-symbolic",
            BackupStatus::Error => "backup-error-symbolic"
        };

        let status_text = match status {
            BackupStatus::Unmodified => "unmodified",
            BackupStatus::Modified => "modified",
            BackupStatus::Error => "read error"
        };

        // Build BackupObject
        glib::Object::builder()
            .property("filename", filename)
            .property("status-icon", status_icon)
            .property("status-text", status_text)
            .build()
    }
}
