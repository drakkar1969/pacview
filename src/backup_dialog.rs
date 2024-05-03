use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use adw::prelude::AdwDialogExt;

use titlecase::titlecase;

use crate::pkg_object::PkgObject;
use crate::backup_object::BackupObject;
use crate::utils::Utils;

//------------------------------------------------------------------------------
// MODULE: BackupDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/backup_dialog.ui")]
    pub struct BackupDialog {
        #[template_child]
        pub(super) header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) status_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub(super) status_model: TemplateChild<gtk::StringList>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) section_factory: TemplateChild<gtk::SignalListItemFactory>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupDialog {
        const NAME: &'static str = "BackupDialog";
        type Type = super::BackupDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            BackupObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BackupDialog {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
        }
    }

    impl WidgetImpl for BackupDialog {}
    impl AdwDialogImpl for BackupDialog {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: BackupDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct BackupDialog(ObjectSubclass<imp::BackupDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BackupDialog {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // View header factory signals
        imp.section_factory.connect_setup(|_, item| {
            let item = item
                .downcast_ref::<gtk::ListHeader>()
                .expect("Could not downcast to 'ListHeader'");

            let label = gtk::Label::new(None);
            label.set_xalign(0.0);

            item.set_child(Some(&label));
        });

        imp.section_factory.connect_bind(|_, item| {
            let item = item
                .downcast_ref::<gtk::ListHeader>()
                .expect("Could not downcast to 'ListHeader'");

            let label = item.child()
                .and_downcast::<gtk::Label>()
                .expect("Could not downcast to 'Label'");

            let obj = item.item()
                .and_downcast::<BackupObject>()
                .expect("Could not downcast to 'BackupObject'");

            label.set_label(&format!("{} ({})",
                obj.package().unwrap_or("(Unknown)".to_string()),
                item.n_items()
            ));
        });

        // Status dropdown selected property notify signal
        imp.status_dropdown.connect_selected_item_notify(clone!(@weak imp => move |dropdown| {
            if dropdown.selected() == 0 {
                imp.status_filter.set_search(None);
            } else {
                let sel_text = dropdown.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .map(|obj| obj.string());

                imp.status_filter.set_search(sel_text.as_deref());
            }

            imp.view.grab_focus();
        }));

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(@weak imp => move |_| {
            let item = imp.selection.selected_item()
                .and_downcast::<BackupObject>()
                .expect("Could not downcast to 'BackupObject'");

            Utils::open_file_manager(&item.filename());
        }));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(@weak self as dialog, @weak imp => move |_| {
            let copy_text = imp.selection.iter::<glib::Object>().flatten()
                .map(|item| {
                    let backup = item
                        .downcast::<BackupObject>()
                        .expect("Could not downcast to 'BackupObject'");

                    format!("{package} => {filename} ({status})", package=backup.package().unwrap_or("None".to_string()), filename=backup.filename(), status=backup.status_text())
                })
                .collect::<Vec<String>>()
                .join("\n");

            dialog.clipboard().set_text(&copy_text);
        }));

        // Column view activate signal
        imp.view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.open_button.emit_clicked();
        }));
    }

    //-----------------------------------
    // Update widgets
    //-----------------------------------
    fn update_ui(&self, pkg_snapshot: &[PkgObject]) {
        let imp = self.imp();

        let backup_snapshot: Vec<(String, Vec<(String, String)>)> = pkg_snapshot.iter()
            .filter(|pkg| !pkg.backup().is_empty())
            .map(|pkg| (pkg.name(), pkg.backup()))
            .collect();

        // Spawn thread to compute backup file hashes
        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            let data_list: Vec<(String, String, String, Option<String>)> = backup_snapshot.iter()
                .flat_map(|(name, backup)| {
                    backup.iter()
                        .map(|(filename, hash)| (filename.to_string(), hash.to_string(), name.to_string(), alpm::compute_md5sum(filename.as_str()).ok()))
                })
                .collect();

            sender.send_blocking(data_list).expect("Could not send through channel");
        });

        // Attach thread receiver
        glib::spawn_future_local(clone!(@weak imp => async move {
            while let Ok(data_list) = receiver.recv().await {
                // Populate column view
                let mut status_list: Vec<String> = vec![];

                let backup_list: Vec<BackupObject> = data_list.into_iter()
                    .map(|(filename, hash, name, file_hash)| {
                        let obj = BackupObject::new(&filename, &hash, Some(&name), file_hash.as_deref());

                        status_list.push(titlecase(&obj.status_text()));

                        obj
                    })
                    .collect();

                imp.model.extend_from_slice(&backup_list);

                // Populate status dropdown
                status_list.sort_unstable();
                status_list.dedup();

                imp.status_model.append("All");
                imp.status_model.splice(1, 0, &status_list.iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<&str>>()
                );

                // Bind backup files count to header label
                let backup_len = backup_list.len();

                imp.selection.bind_property("n-items", &imp.header_label.get(), "label")
                    .transform_to(move |_, n_items: u32| Some(format!("Backup Files ({n_items} of {backup_len})")))
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build();

                // Bind backup files count to open/copy button states
                imp.selection.bind_property("n-items", &imp.open_button.get(), "sensitive")
                    .transform_to(|_, n_items: u32| Some(n_items > 0))
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build();

                imp.selection.bind_property("n-items", &imp.copy_button.get(), "sensitive")
                    .transform_to(|_, n_items: u32| Some(n_items > 0))
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build();
            }
        }));
    }
    //-----------------------------------
    // Public show function
    //-----------------------------------
    pub fn show(&self, parent: &impl IsA<gtk::Widget>, pkg_snapshot: &[PkgObject]) {
        self.update_ui(pkg_snapshot);

        self.present(parent);
    }
}

impl Default for BackupDialog {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
