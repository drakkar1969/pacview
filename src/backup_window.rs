use std::collections::HashSet;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use itertools::Itertools;

use crate::pkg_object::{PkgBackup, PkgObject};
use crate::backup_object::{BackupObject, BackupStatus};
use crate::enum_traits::EnumValueExt;
use crate::utils::open_with_default_app;

//------------------------------------------------------------------------------
// MODULE: BackupWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/backup_window.ui")]
    pub struct BackupWindow {
        #[template_child]
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) status_dropdown: TemplateChild<gtk::DropDown>,
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
        pub(super) section_sort_model: TemplateChild<gtk::SortListModel>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) backup_filter: TemplateChild<gtk::EveryFilter>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) section_sorter: TemplateChild<gtk::StringSorter>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupWindow {
        const NAME: &'static str = "BackupWindow";
        type Type = super::BackupWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            BackupObject::ensure_type();

            klass.bind_template();

            klass.add_binding(gdk::Key::Escape, gdk::ModifierType::NO_MODIFIER_MASK, |window| {
                window.close();

                glib::Propagation::Stop
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BackupWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
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
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

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
                    })
                    .collect();

                let section_len = section_map.len();

                Some(format!("{n_items} files in {section_len} package{}",
                    if section_len != 1 {"s"} else {""}
                ))
            })
            .sync_create()
            .build();

        // Bind selected item to open button state
        imp.selection.bind_property("selected-item", &imp.open_button.get(), "sensitive")
            .transform_to(|_, item: Option<glib::Object>| {
                if let Some(object) = item.and_downcast::<BackupObject>() {
                    let status = object.status();

                    Some(status != BackupStatus::Error && status != BackupStatus::All)
                } else {
                    Some(false)
                }
            })
            .sync_create()
            .build();

        // Bind backup files count to copy button state
        imp.filter_model.bind_property("n-items", &imp.copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search entry search started signal
        imp.search_entry.connect_search_started(|entry| {
            if !entry.has_focus() {
                entry.grab_focus();
            }
        });

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_filter.set_search(Some(&entry.text()));
            }
        ));

        // Status dropdown selected property notify signal
        imp.status_dropdown.connect_selected_item_notify(clone!(
            #[weak] imp,
            move |dropdown| {
                let status = BackupStatus::from_repr(dropdown.selected()).unwrap_or_default();

                if status == BackupStatus::All {
                    imp.status_filter.set_search(None);
                } else {
                    imp.status_filter.set_search(Some(&status.name()));
                }

                imp.view.scroll_to(imp.selection.selected(), None, gtk::ListScrollFlags::FOCUS, None);

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

                open_with_default_app(&item.filename());
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            move |_| {
                let mut copy_text = "## Backup Files\n|Filename|Status|\n|---|---|\n".to_string();

                let mut package = String::from("");

                copy_text.push_str(&imp.selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        let backup = item
                            .downcast::<BackupObject>()
                            .expect("Could not downcast to 'BackupObject'");

                        let mut line = String::from("");

                        let backup_package = backup.package();

                        if backup_package != package {
                            line.push_str(&format!("|**{package}**||\n",
                                package=backup_package
                            ));

                            package = backup_package;
                        }

                        line.push_str(&format!("|{filename}|{status}|",
                            filename=backup.filename(),
                            status=backup.status_text()
                        ));

                        line
                    })
                    .join("\n"));

                window.clipboard().set_text(&copy_text);
            }
        ));

        // Column view activate signal
        imp.view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }
            }
        ));
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, installed_snapshot: &[PkgObject]) {
        let imp = self.imp();

        self.present();

        // Define local enum
        enum BackupResult {
            Backup(PkgBackup),
            End
        }

        // Disable sorting/filtering
        let section_sorter = imp.section_sort_model.section_sorter();
        let filter = imp.filter_model.filter();

        imp.section_sort_model.set_section_sorter(None::<&gtk::Sorter>);
        imp.filter_model.set_filter(None::<&gtk::Filter>);

        // Get backup list
        let backup_list: Vec<PkgBackup> = installed_snapshot.iter()
            .flat_map(|pkg| pkg.backup().iter().cloned())
            .collect();

        // Spawn task to populate column view
        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(clone!(
            move || {
                for backup in backup_list {
                    sender.send_blocking(BackupResult::Backup(backup))
                        .expect("Could not send through channel");
                }

                sender.send_blocking(BackupResult::End).expect("Could not send through channel");
            }
        ));

        // Attach channel receiver
        glib::spawn_future_local(clone!(
            #[weak] imp,
            async move {
                while let Ok(result) = receiver.recv().await {
                    match result {
                        // Append backup to column view
                        BackupResult::Backup(backup) => {
                            imp.model.append(&BackupObject::new(&backup));
                        },
                        // Enable sorting/filtering and select first item in column view
                        BackupResult::End => {
                            imp.status_dropdown.set_selected(0);

                            imp.section_sort_model.set_section_sorter(section_sorter.as_ref());
                            imp.filter_model.set_filter(filter.as_ref());

                            imp.selection.set_selected(0);
                            imp.view.scroll_to(0, None, gtk::ListScrollFlags::FOCUS, None);
                        }
                    };
                }
            }
        ));
    }
}
