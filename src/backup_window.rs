use std::collections::HashSet;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use crate::pkg_object::PkgObject;
use crate::backup_object::{BackupObject, BackupStatus};
use crate::traits::EnumClassExt;
use crate::utils::open_file_manager;

//------------------------------------------------------------------------------
// MODULE: BackupWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/backup_window.ui")]
    pub struct BackupWindow {
        #[template_child]
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) status_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub(super) status_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) section_factory: TemplateChild<gtk::BuilderListItemFactory>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupWindow {
        const NAME: &'static str = "BackupWindow";
        type Type = super::BackupWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            BackupObject::ensure_type();

            klass.bind_template();

            klass.add_shortcut(&gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string("Escape"),
                Some(gtk::CallbackAction::new(|widget, _| {
                    let window = widget
                        .downcast_ref::<crate::backup_window::BackupWindow>()
                        .expect("Could not downcast to 'BackupWindow'");

                    window.close();

                    glib::Propagation::Proceed
                }))
            ))
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BackupWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for BackupWindow {}
    impl WindowImpl for BackupWindow {}
    impl AdwWindowImpl for BackupWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: BackupWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct BackupWindow(ObjectSubclass<imp::BackupWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl BackupWindow {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Populate status dropdown
        imp.status_model.append(&gtk::StringObject::new("All"));

        let enum_class = BackupStatus::enum_class();

        imp.status_model.extend_from_slice(&enum_class.values().iter()
            .map(|value| gtk::StringObject::new(value.name()))
            .collect::<Vec<gtk::StringObject>>()
        );

        // Bind backup files count to header sub label
        imp.filter_model.bind_property("n-items", &imp.header_sub_label.get(), "label")
            .transform_to(move |binding, n_items: u32| {
                let filter_model = binding.source()
                    .and_downcast::<gtk::FilterListModel>()
                    .expect("Could not downcast to 'FilterListModel'");

                let section_map: HashSet<String> = filter_model.iter::<glib::Object>().flatten()
                    .map(|item| {
                        item
                            .downcast::<BackupObject>()
                            .expect("Could not downcast to 'BackupObject'")
                            .package()
                            .unwrap_or_default()
                    })
                    .collect();

                let section_len = section_map.len();

                Some(format!("{n_items} files in {section_len} package{}", if section_len != 1 {"s"} else {""}))
            })
            .sync_create()
            .build();

        // Bind backup files count to open/copy button states
        imp.filter_model.bind_property("n-items", &imp.open_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        imp.filter_model.bind_property("n-items", &imp.copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Status dropdown selected property notify signal
        imp.status_dropdown.connect_selected_item_notify(clone!(
            #[weak] imp,
            move |dropdown| {
                if dropdown.selected() == 0 {
                    imp.status_filter.set_search(None);
                } else {
                    let sel_text = dropdown.selected_item()
                        .and_downcast::<gtk::StringObject>()
                        .map(|obj| obj.string());

                    imp.status_filter.set_search(sel_text.as_deref());
                }

                imp.view.grab_focus();
            }
        ));

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Could not downcast to 'BackupObject'");

                open_file_manager(&item.filename());
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            move |_| {
                let copy_text = imp.selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        let backup = item
                            .downcast::<BackupObject>()
                            .expect("Could not downcast to 'BackupObject'");

                        format!("{package} => {filename} ({status})", package=backup.package().unwrap_or("None".to_string()), filename=backup.filename(), status=backup.status_text())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(&copy_text);
            }
        ));

        // Column view activate signal
        imp.view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                imp.open_button.emit_clicked();
            }
        ));
    }

    //-----------------------------------
    // Show window
    //-----------------------------------
    pub fn show(&self, pkg_snapshot: &[PkgObject]) {
        let imp = self.imp();

        self.present();

        let pkg_snapshot = pkg_snapshot.to_vec();

        // Spawn thread to populate column view
        glib::spawn_future_local(clone!(
            #[weak] imp,
            async move {
                let backup_list: Vec<BackupObject> = pkg_snapshot.iter()
                    .flat_map(|pkg| { pkg.backup().into_iter()
                        .map(|(filename, hash)| BackupObject::new(&filename, &hash, Some(&pkg.name()), alpm::compute_md5sum(filename.as_str()).as_deref()))
                    })
                    .collect();

                imp.model.extend_from_slice(&backup_list);
            }
        ));
    }
}
