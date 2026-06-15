use std::cell::{RefCell, OnceCell};
use std::marker::PhantomData;
use std::io;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use strum::{FromRepr, AsRefStr};

use crate::{
    pkg_object::PkgBackup,
    utils::{Paths, Pacman, TokioUtils}
};

//------------------------------------------------------------------------------
// ENUM: BackupStatus
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, FromRepr, AsRefStr)]
#[strum(serialize_all = "lowercase")]
#[repr(u32)]
#[enum_type(name = "BackupStatus")]
pub enum BackupStatus {
    All,
    Modified,
    #[strum(serialize = "access denied")]
    #[enum_value(name = "Access Denied")]
    Locked,
    #[default]
    Unmodified,
}

//------------------------------------------------------------------------------
// MODULE: BackupObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::BackupObject)]
    pub struct BackupObject {
        // Read-write properties, construct only
        #[property(get, set, construct_only)]
        path: RefCell<String>,
        #[property(get, set, construct_only)]
        hash: RefCell<String>,
        #[property(get, set, construct_only)]
        package: RefCell<String>,

        // Read only properties
        #[property(get = Self::status, builder(BackupStatus::default()))]
        status: OnceCell<BackupStatus>,

        #[property(get = Self::status_css_classes)]
        status_css_classes: PhantomData<Vec<String>>,
        #[property(get = Self::status_text)]
        status_text: PhantomData<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupObject {
        const NAME: &'static str = "BackupObject";
        type Type = super::BackupObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for BackupObject {}

    impl BackupObject {
        //---------------------------------------
        // Property getters
        //---------------------------------------
        fn status(&self) -> BackupStatus {
            *self.status.get_or_init(|| {
                let path = Pacman::config().root_dir.clone() + &self.path.borrow();

                let file_hash = alpm::compute_md5sum(path.as_str());

                file_hash.map_or(BackupStatus::Locked, |file_hash| {
                    if file_hash == self.obj().hash() {
                        BackupStatus::Unmodified
                    } else {
                        BackupStatus::Modified
                    }
                })
            })
        }

        fn status_css_classes(&self) -> Vec<String> {
            match self.status() {
                BackupStatus::Modified => vec!["tag", "warning"],
                BackupStatus::Locked => vec!["tag", "error"],
                _ => vec![]
            }
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
        }

        fn status_text(&self) -> String {
            let status = self.status();

            if status == BackupStatus::Modified || status == BackupStatus::Locked {
                status.as_ref().to_owned()
            } else {
                String::new()
            }
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
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(backup: &PkgBackup, package: &str) -> Self {
        glib::Object::builder()
            .property("path", backup.path())
            .property("hash", backup.hash())
            .property("package", package)
            .build()
    }

    //---------------------------------------
    // Async compare with original function
    //---------------------------------------
    #[allow(clippy::future_not_send)]
    pub async fn compare_with_original(&self) -> io::Result<()> {
        let meld = Paths::meld().as_ref()
            .map_err(|_| io::Error::other("Meld not found"))?;

        let paccat = Paths::paccat().as_ref()
            .map_err(|_| io::Error::other("Paccat not found"))?;

        let path = Pacman::config().root_dir.clone() + &self.path();

        // Download original file content with paccat
        let (status, content) = TokioUtils::run(paccat, &[&self.package(), "--", &path], None)
            .await?;

        if status != Some(0) {
            return Err(io::Error::other("Paccat error"))
        }

        // Compare backup file with original content
        TokioUtils::spawn_pipe_stdin(meld, &["/dev/stdin", &path], &content)
            .await?;

        Ok(())
    }
}
