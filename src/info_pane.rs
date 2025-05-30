use std::cell::RefCell;
use std::marker::PhantomData;
use std::collections::HashMap;
use std::borrow::Cow;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::closure_local;
use glib::clone;

use crate::package_view::AUR_PKGS;
use crate::text_widget::{TextWidget, INSTALLED_LABEL};
use crate::info_row::{PropID, PropType, ValueType, InfoRow};
use crate::history_list::HistoryList;
use crate::pkg_data::{PkgFlags, PkgValidation};
use crate::pkg_object::PkgObject;
use crate::hash_window::HashWindow;
use crate::backup_object::{BackupObject, BackupStatus};
use crate::utils::app_info;

//------------------------------------------------------------------------------
// MODULE: InfoPane
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoPane)]
    #[template(resource = "/com/github/PacView/ui/info_pane.ui")]
    pub struct InfoPane {
        #[template_child]
        pub(super) title_widget: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(super) prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) next_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) tab_switcher: TemplateChild<adw::InlineViewSwitcher>,
        #[template_child]
        pub(super) tab_stack: TemplateChild<adw::ViewStack>,

        #[template_child]
        pub(super) info_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) info_hashes_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) info_copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) files_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) files_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) files_search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) files_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) files_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) files_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) files_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) files_filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) files_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) files_filter: TemplateChild<gtk::StringFilter>,

        #[template_child]
        pub(super) log_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) log_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) log_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) log_selection: TemplateChild<gtk::NoSelection>,

        #[template_child]
        pub(super) cache_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) cache_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) cache_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) cache_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) cache_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) cache_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) cache_selection: TemplateChild<gtk::SingleSelection>,

        #[template_child]
        pub(super) backup_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) backup_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) backup_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) backup_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) backup_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) backup_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) backup_selection: TemplateChild<gtk::SingleSelection>,

        #[property(get = Self::pkg, set = Self::set_pkg, nullable)]
        pkg: PhantomData<Option<PkgObject>>,

        pub(super) info_row_map: RefCell<HashMap<PropID, InfoRow>>,

        pub(super) pkg_history: RefCell<HistoryList>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoPane {
        const NAME: &'static str = "InfoPane";
        type Type = super::InfoPane;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            BackupObject::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InfoPane {
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

    impl WidgetImpl for InfoPane {}
    impl BinImpl for InfoPane {}
    impl InfoPane {
        //---------------------------------------
        // Property getter/setter
        //---------------------------------------
        fn pkg(&self) -> Option<PkgObject> {
            self.pkg_history.borrow().selected_item()
        }

        fn set_pkg(&self, pkg: Option<&PkgObject>) {
            self.pkg_history.borrow().init(pkg);

            self.main_stack.set_visible_child_name(
                if pkg.is_some() { "properties" } else { "empty" }
            );

            self.tab_switcher.set_sensitive(pkg.is_some());

            self.info_hashes_button.set_sensitive(
                pkg.is_some_and(|pkg| {
                    let validation = pkg.validation();

                    !(validation.intersects(PkgValidation::UNKNOWN) ||
                    validation.intersects(PkgValidation::NONE))
                })
            );

            self.obj().update_display();
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoPane
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoPane(ObjectSubclass<imp::InfoPane>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl InfoPane {
    //---------------------------------------
    // InfoRow pkg link handler
    //---------------------------------------
    fn pkg_link_handler(&self, pkg_name: &str, pkg_version: &str) {
        AUR_PKGS.with_borrow(|aur_pkgs| {
            // Find link package in pacman databases
            let pkg_link = pkg_name.to_owned() + pkg_version;

            let pkg = PkgObject::find_satisfier(&pkg_link);

            // Find link package in AUR search results
            let new_pkg = pkg.as_ref()
                .or_else(|| {
                    aur_pkgs.iter()
                        .find(|&pkg| pkg.name() == pkg_name)
                        .or_else(|| {
                            aur_pkgs.iter()
                                .find(|&pkg| pkg.provides().iter().any(|s| s == &pkg_link))
                        })
                });

            // If link package found
            if let Some(new_pkg) = new_pkg {
                let pkg_history = self.imp().pkg_history.borrow();

                // If link package is in infopane history, select it
                // Otherwise append it after current history package
                pkg_history.select_or_append_next(new_pkg);

                // Display link package
                self.update_display();
            }
        });
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Add info rows
        self.add_info_row(PropID::Name, PropType::Title);
        self.add_info_row(PropID::Version, PropType::Text);
        self.add_info_row(PropID::Description, PropType::Text);
        self.add_info_row(PropID::Popularity, PropType::Text);
        self.add_info_row(PropID::OutOfDate, PropType::Error);
        self.add_info_row(PropID::PackageUrl, PropType::Link);
        self.add_info_row(PropID::Url, PropType::Link);
        self.add_info_row(PropID::Status, PropType::Text);
        self.add_info_row(PropID::Repository, PropType::Text);
        self.add_info_row(PropID::Groups, PropType::Text);
        self.add_info_row(PropID::Dependencies, PropType::LinkList);
        self.add_info_row(PropID::Optional, PropType::LinkList);
        self.add_info_row(PropID::Make, PropType::LinkList);
        self.add_info_row(PropID::RequiredBy, PropType::LinkList);
        self.add_info_row(PropID::OptionalFor, PropType::LinkList);
        self.add_info_row(PropID::Provides, PropType::Text);
        self.add_info_row(PropID::ConflictsWith, PropType::LinkList);
        self.add_info_row(PropID::Replaces, PropType::LinkList);
        self.add_info_row(PropID::Licenses, PropType::Text);
        self.add_info_row(PropID::Architecture, PropType::Text);
        self.add_info_row(PropID::Packager, PropType::Packager);
        self.add_info_row(PropID::BuildDate, PropType::Text);
        self.add_info_row(PropID::InstallDate, PropType::Text);
        self.add_info_row(PropID::DownloadSize, PropType::Text);
        self.add_info_row(PropID::InstalledSize, PropType::Text);
        self.add_info_row(PropID::InstallScript, PropType::Text);
        self.add_info_row(PropID::Validation, PropType::Text);

        // Set files search entry key capture widget
        imp.files_search_entry.set_key_capture_widget(Some(&imp.files_view.get()));

        // Bind history list properties to widgets
        let pkg_history = imp.pkg_history.borrow();

        pkg_history.bind_property("can-select-prev", &imp.prev_button.get(), "sensitive")
            .sync_create()
            .build();

        pkg_history.bind_property("can-select-next", &imp.next_button.get(), "sensitive")
            .sync_create()
            .build();
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Previous button clicked signal
        imp.prev_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                infopane.display_prev();
            }
        ));

        // Next button clicked signal
        imp.next_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                infopane.display_next();
            }
        ));

        // Info hashes button clicked signal
        imp.info_hashes_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                if let Some(pkg) = infopane.pkg() {
                    let parent = infopane.root()
                        .and_downcast::<gtk::Window>()
                        .expect("Failed to downcast to 'GtkWindow'");

                    let hash_window = HashWindow::new(&parent);

                    hash_window.show(&pkg);
                }
            }
        ));

        // Info copy button clicked signal
        imp.info_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                let body = {
                    let mut properties: Vec<String> = vec![];

                    let mut child = infopane.imp().info_listbox.first_child();

                    while let Some(row) = child.and_downcast::<InfoRow>() {
                        if row.is_visible() {
                            let label = row.label();
                            let value = row.value();

                            if !(label.is_empty() || value.is_empty()) {
                                properties.push(format!("- **{label}** : {value}"));
                            }
                        }

                        child = row.next_sibling();
                    }

                    properties.join("\n")
                };

                infopane.clipboard().set_text(&format!("## Package Information\n{body}"));
            }
        ));

        // Files search entry search started signal
        imp.files_search_entry.connect_search_started(|entry| {
            if !entry.has_focus() {
                entry.grab_focus();
            }
        });

        // Files search entry search changed signal
        imp.files_search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.files_filter.set_search(Some(&entry.text()));
            }
        ));

        // Files open button clicked signal
        imp.files_open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.files_selection.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .expect("Failed to downcast to 'StringObject'");

                app_info::open_with_default_app(&item.string());
            }
        ));

        // Files copy button clicked signal
        imp.files_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                let body = infopane.imp().files_selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        item
                            .downcast::<gtk::StringObject>()
                            .expect("Failed to downcast to 'StringObject'")
                            .string()
                    })
                    .collect::<Vec<glib::GString>>()
                    .join("\n");

                infopane.clipboard().set_text(
                    &format!("## {}\n|Files|\n|---|\n{body}", infopane.pkg().unwrap().name())
                );
            }
        ));

        // Files listview activate signal
        imp.files_view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.files_open_button.is_sensitive() {
                    imp.files_open_button.emit_clicked();
                }
            }
        ));

        // Files selection items changed signal
        imp.files_selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.files_count_label.set_label(&n_items.to_string());
                imp.files_open_button.set_sensitive(n_items > 0);
                imp.files_copy_button.set_sensitive(n_items > 0);
            }
        ));

        // Log copy button clicked signal
        imp.log_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                let body = infopane.imp().log_model.iter::<gtk::StringObject>().flatten()
                    .map(|item| item.string())
                    .collect::<Vec<glib::GString>>()
                    .join("\n");

                infopane.clipboard().set_text(
                    &format!("## {}\n|Log Messages|\n|---|\n{body}", infopane.pkg().unwrap().name())
                );
            }
        ));

        // Log selection items changed signal
        imp.log_selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.log_copy_button.set_sensitive(n_items > 0);
            }
        ));

        // Cache open button clicked signal
        imp.cache_open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.cache_selection.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .expect("Failed to downcast to 'StringObject'");

                app_info::open_containing_folder(&item.string());
            }
        ));

        // Cache copy button clicked signal
        imp.cache_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                let body = infopane.imp().cache_model.iter::<gtk::StringObject>().flatten()
                    .map(|item| item.string())
                    .collect::<Vec<glib::GString>>()
                    .join("\n");

                infopane.clipboard().set_text(
                    &format!("## {}\n|Cache Files|\n|---|\n{body}", infopane.pkg().unwrap().name())
                );
            }
        ));

        // Cache listview activate signal
        imp.cache_view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.cache_open_button.is_sensitive() {
                    imp.cache_open_button.emit_clicked();
                }
            }
        ));

        // Cache selection items changed signal
        imp.cache_selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.cache_count_label.set_label(&n_items.to_string());
                imp.cache_open_button.set_sensitive(n_items > 0);
                imp.cache_copy_button.set_sensitive(n_items > 0);
            }
        ));

        // Backup open button clicked signal
        imp.backup_open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.backup_selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'");

                    app_info::open_with_default_app(&item.filename());
            }
        ));

        // Backup copy button clicked signal
        imp.backup_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                let body = infopane.imp().backup_model.iter::<BackupObject>().flatten()
                    .map(|item| format!("{}|{}", item.filename(), item.status_text()))
                    .collect::<Vec<String>>()
                    .join("\n");

                infopane.clipboard().set_text(
                    &format!("## {}\n|Backup Files|Status|\n|---|---|\n{body}", infopane.pkg().unwrap().name())
                );
            }
        ));

        // Backup listview activate signal
        imp.backup_view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.backup_open_button.is_sensitive() {
                    imp.backup_open_button.emit_clicked();
                }
            }
        ));

        // Backup selection items changed signal
        imp.backup_selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.backup_count_label.set_label(&n_items.to_string());
                imp.backup_copy_button.set_sensitive(n_items > 0);
            }
        ));

        // Backup selection selected item property notify signal
        imp.backup_selection.connect_selected_item_notify(clone!(
            #[weak] imp,
            move |selection| {
                let status = selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::Locked, |backup| backup.status());

                imp.backup_open_button.set_sensitive(
                    status != BackupStatus::Locked && status != BackupStatus::All
                );
            }
        ));
    }

    //---------------------------------------
    // Add info row function
    //---------------------------------------
    fn add_info_row(&self, id: PropID, ptype: PropType) {
        let imp = self.imp();

        let row = InfoRow::new(id, ptype);

        row.set_pkg_link_handler(closure_local!(
            #[weak(rename_to = infopane)] self,
            move |_: TextWidget, pkg_name: &str, pkg_version: &str| {
                infopane.pkg_link_handler(pkg_name, pkg_version);
            }
        ));

        imp.info_listbox.append(&row);

        imp.info_row_map.borrow_mut().insert(id, row);
    }

    //---------------------------------------
    // Set info row function
    //---------------------------------------
    fn set_info_row(&self, id: PropID, value: ValueType) {
        if let Some(row) = self.imp().info_row_map.borrow().get(&id) {
            row.set_value(value);
        }
    }

    //---------------------------------------
    // Installed optdeps function
    //---------------------------------------
    fn installed_optdeps(flags: PkgFlags, optdepends: &[String]) -> Cow<'_, [String]> {
        if !optdepends.is_empty() && flags.intersects(PkgFlags::INSTALLED) {
            optdepends.iter()
                .map(|dep| {
                    if dep.split_once([':'])
                        .and_then(|(name, _)| PkgObject::has_local_satisfier(name))
                        .unwrap_or_default()
                    {
                        dep.to_string() + INSTALLED_LABEL
                    } else {
                        dep.to_string()
                    }
                })
                .collect()
        } else {
            Cow::Borrowed(optdepends)
        }
    }

    //---------------------------------------
    // Package validation function
    //---------------------------------------
    fn validation(&self, flags: PkgValidation) -> String {
        let validation_flags_class = glib::FlagsClass::new::<PkgValidation>();

        flags.iter()
            .map(|flag| {
                validation_flags_class
                    .value(flag.bits())
                    .map_or("NONE", glib::FlagsValue::name)
            })
            .collect::<Vec<&str>>()
            .join(" | ")
    }

    //---------------------------------------
    // Display helper functions
    //---------------------------------------
    fn update_info_listbox(&self, pkg: &PkgObject) {
        // Name
        self.set_info_row(PropID::Name, ValueType::Str(&pkg.name()));

        // Version
        self.set_info_row(PropID::Version,
            ValueType::StrIcon(
                &pkg.version(),
                pkg.flags().intersects(PkgFlags::UPDATES).then_some("pkg-update")
            )
        );

        // Description
        self.set_info_row(PropID::Description, ValueType::StrOpt(pkg.description()));

        // Popularity
        self.set_info_row(PropID::Popularity, ValueType::StrOpt(pkg.popularity()));

        // Out of Date
        self.set_info_row(PropID::OutOfDate, ValueType::StrOptNum(pkg.out_of_date_string(), pkg.out_of_date()));

        // Package URL
        self.set_info_row(PropID::PackageUrl, ValueType::StrOpt(pkg.package_url()));

        // URL
        self.set_info_row(PropID::Url, ValueType::StrOpt(pkg.url()));

        // Licenses
        self.set_info_row(PropID::Licenses, ValueType::StrOpt(pkg.licenses()));

        // Status
        let status_icon = pkg.status_icon();

        self.set_info_row(PropID::Status,
            ValueType::StrIcon(
                &pkg.status(),
                pkg.flags().intersects(PkgFlags::INSTALLED).then_some(&status_icon)
            )
        );

        // Repository
        self.set_info_row(PropID::Repository, ValueType::Str(&pkg.repository()));

        // Groups
        self.set_info_row(PropID::Groups, ValueType::StrOpt(&pkg.groups()));

        // Depends
        self.set_info_row(PropID::Dependencies, ValueType::Vec(pkg.depends()));

        // Optdepends
        self.set_info_row(PropID::Optional, ValueType::VecOpt(&Self::installed_optdeps(pkg.flags(), pkg.optdepends())));

        // Makedepends
        self.set_info_row(PropID::Make, ValueType::VecOpt(pkg.makedepends()));

        // Required by
        self.set_info_row(PropID::RequiredBy, ValueType::Vec(pkg.required_by()));

        // Optional for
        self.set_info_row(PropID::OptionalFor, ValueType::VecOpt(pkg.optional_for()));

        // Provides
        self.set_info_row(PropID::Provides, ValueType::VecOpt(pkg.provides()));

        // Conflicts
        self.set_info_row(PropID::ConflictsWith, ValueType::VecOpt(pkg.conflicts()));

        // Replaces
        self.set_info_row(PropID::Replaces, ValueType::VecOpt(pkg.replaces()));

        // Architecture
        self.set_info_row(PropID::Architecture, ValueType::StrOpt(pkg.architecture()));

        // Packager
        self.set_info_row(PropID::Packager, ValueType::Str(pkg.packager()));

        // Build date
        self.set_info_row(PropID::BuildDate, ValueType::StrOptNum(pkg.build_date_string(), pkg.build_date()));

        // Install date
        self.set_info_row(PropID::InstallDate, ValueType::StrOptNum(pkg.install_date_string(), pkg.install_date()));

        // Download size
        self.set_info_row(PropID::DownloadSize, ValueType::StrOptNum(pkg.download_size_string(), pkg.download_size()));

        // Installed size
        self.set_info_row(PropID::InstalledSize, ValueType::Str(&pkg.install_size_string()));

        // Has script
        self.set_info_row(PropID::InstallScript, ValueType::StrOpt(pkg.has_script()));

        // Validation
        self.set_info_row(PropID::Validation, ValueType::Str(&self.validation(pkg.validation())));
    }

    fn update_files_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Populate files view
        let files_list: Vec<gtk::StringObject> = pkg.files().iter()
            .map(|file| gtk::StringObject::new(file))
            .collect();

        imp.files_model.splice(0, imp.files_model.n_items(), &files_list);
    }

    fn update_log_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Populate log view
        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] pkg,
            async move {
                let log_lines: Vec<gtk::StringObject> = pkg.log_future().await.iter()
                    .map(|line| gtk::StringObject::new(line))
                    .collect();

                imp.log_model.splice(0, imp.log_model.n_items(), &log_lines);
            }
        ));
    }

    fn update_cache_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Populate cache view
        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] pkg,
            async move {
                let cache_list: Vec<gtk::StringObject> = pkg.cache_future().await.iter()
                    .map(|cache_file| gtk::StringObject::new(cache_file))
                    .collect();

                imp.cache_model.splice(0, imp.cache_model.n_items(), &cache_list);
            }
        ));
    }

    fn update_backup_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Populate backup view
        let backup_list: Vec<BackupObject> = pkg.backup().iter()
            .map(BackupObject::new)
            .collect();

        imp.backup_model.splice(0, imp.backup_model.n_items(), &backup_list);
    }

    //---------------------------------------
    // Public display functions
    //---------------------------------------
    pub fn update_display(&self) {
        let imp = self.imp();

        // Clear header bar title
        imp.title_widget.set_title("");

        // Clear files search entry
        imp.files_search_entry.set_text("");

        // If package is not none, display it
        if let Some(pkg) = self.pkg() {
            // Set header bar title
            let pkg_history = imp.pkg_history.borrow();

            let title = if pkg_history.n_items() > 1 {
                format!("{}  \u{2022}  {}/{}", pkg.name(), pkg_history.selected() + 1, pkg_history.n_items())
            } else {
                pkg.name()
            };

            imp.title_widget.set_title(&title);

            // Populate info listbox
            self.update_info_listbox(&pkg);

            // Populate files/log/cache/backup views
            self.update_files_view(&pkg);

            self.update_log_view(&pkg);

            self.update_cache_view(&pkg);

            self.update_backup_view(&pkg);
        }
    }

    pub fn display_prev(&self) {
        self.imp().pkg_history.borrow().select_previous();

        self.update_display();
    }

    pub fn display_next(&self) {
        self.imp().pkg_history.borrow().select_next();

        self.update_display();
    }

    //---------------------------------------
    // Public display functions
    //---------------------------------------
    pub fn set_visible_tab(&self, tab: &str) {
        let imp = self.imp();

        if imp.tab_switcher.is_sensitive() {
            imp.tab_stack.set_visible_child_name(tab);
        }
    }
}

impl Default for InfoPane {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
