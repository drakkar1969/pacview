use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::closure_local;
use glib::clone;

use itertools::Itertools;

use crate::package_view::AUR_PKGS;
use crate::text_widget::{TextWidget, PropType, INSTALLED_LABEL, LINK_SPACER};
use crate::property_value::PropertyValue;
use crate::pkg_history::PkgHistory;
use crate::pkg_object::{PkgObject, PkgFlags};
use crate::backup_object::{BackupObject, BackupStatus};
use crate::enum_traits::EnumExt;
use crate::utils::open_with_default_app;

//------------------------------------------------------------------------------
// ENUM: PropID
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropID")]
enum PropID {
    #[enum_value(name = "Name")]
    Name,
    #[enum_value(name = "Version")]
    Version,
    #[enum_value(name = "Description")]
    Description,
    #[enum_value(name = "Package URL")]
    PackageUrl,
    #[enum_value(name = "URL")]
    Url,
    #[enum_value(name = "Status")]
    Status,
    #[enum_value(name = "Repository")]
    Repository,
    #[enum_value(name = "Groups")]
    Groups,
    #[enum_value(name = "Dependencies")]
    Dependencies,
    #[enum_value(name = "Optional")]
    Optional,
    #[enum_value(name = "Make")]
    Make,
    #[enum_value(name = "Required By")]
    RequiredBy,
    #[enum_value(name = "Optional For")]
    OptionalFor,
    #[enum_value(name = "Provides")]
    Provides,
    #[enum_value(name = "Conflicts With")]
    ConflictsWith,
    #[enum_value(name = "Replaces")]
    Replaces,
    #[enum_value(name = "Licenses")]
    Licenses,
    #[enum_value(name = "Architecture")]
    Architecture,
    #[enum_value(name = "Packager")]
    Packager,
    #[enum_value(name = "Build Date")]
    BuildDate,
    #[enum_value(name = "Install Date")]
    InstallDate,
    #[enum_value(name = "Download Size")]
    DownloadSize,
    #[enum_value(name = "Installed Size")]
    InstalledSize,
    #[enum_value(name = "Install Script")]
    InstallScript,
    #[enum_value(name = "SHA256 Sum")]
    SHA256Sum,
}

impl EnumExt for PropID {}

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
        pub(super) tab_switcher: TemplateChild<adw::ViewSwitcher>,
        #[template_child]
        pub(super) tab_stack: TemplateChild<adw::ViewStack>,

        #[template_child]
        pub(super) info_listbox: TemplateChild<gtk::ListBox>,
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
        _pkg: RefCell<Option<PkgObject>>,
        #[property(get, set)]
        property_max_lines: Cell<i32>,

        pub(super) property_map: RefCell<HashMap<PropID, PropertyValue>>,

        pub(super) pkg_history: RefCell<PkgHistory>,
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
        // Custom property getters/setters
        //---------------------------------------
        pub fn pkg(&self) -> Option<PkgObject> {
            self.pkg_history.borrow().selected_item()
        }

        pub fn set_pkg(&self, pkg: Option<&PkgObject>) {
            self.pkg_history.borrow().init(pkg);

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
    // New function
    //---------------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //---------------------------------------
    // PropertyValue pkg link handler
    //---------------------------------------
    fn pkg_link_handler(&self, pkg_name: &str, pkg_version: &str) {
        AUR_PKGS.with_borrow(|aur_pkgs| {
            // Find link package in pacman databases
            let pkg_link = format!("{pkg_name}{pkg_version}");

            let pkg = PkgObject::find_satisfier(&pkg_link, true);

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
                let history = self.imp().pkg_history.borrow();

                // If link package is in infopane history, select it
                if !history.set_selected_item(new_pkg) {
                    // If history package is not the last one in history, truncate history list and append link package
                    history.truncate_and_append(new_pkg);
                }

                // Display link package
                self.update_display();
            }
        });
    }

    //---------------------------------------
    // Add property function
    //---------------------------------------
    fn add_property(&self, id: PropID, ptype: PropType) {
        let imp = self.imp();

        let property = PropertyValue::new(ptype, &id.name());
        property.add_css_class("property-value");

        if id == PropID::Version {
            property.set_icon_css_class("success", true);
        }

        property.set_pkg_link_handler(closure_local!(
            #[watch(rename_to = infopane)] self,
            move |_: TextWidget, pkg_name: &str, pkg_version: &str| {
                infopane.pkg_link_handler(pkg_name, pkg_version);
            }
        ));

        self.bind_property("property-max-lines", &property, "max-lines")
            .sync_create()
            .build();

        imp.info_listbox.append(&property);

        imp.property_map.borrow_mut().insert(id, property);
    }

    //---------------------------------------
    // Set property functions
    //---------------------------------------
    fn set_string_property(&self, id: PropID, visible: bool, value: &str, icon: Option<&str>) {
        if let Some(property) = self.imp().property_map.borrow().get(&id) {
            property.set_visible(visible);

            if visible {
                property.set_icon(icon);
                property.set_value(value);
            }

            if id == PropID::Status {
                property.set_icon_css_class("error", property.icon().unwrap_or_default() == "pkg-orphan");
            }
        }
    }

    fn set_vec_property(&self, id: PropID, visible: bool, value: &[String], icon: Option<&str>) {
        self.set_string_property(id, visible, &value.join(LINK_SPACER), icon);
    }

    //---------------------------------------
    // Get installed optdeps function
    //---------------------------------------
    fn installed_optdeps(optdepends: &[String]) -> Vec<String> {
        optdepends.iter()
            .map(|dep| {
                let mut dep = dep.to_string();

                if dep.split_once(['<', '>', '=', ':'])
                    .and_then(|(name, _)| PkgObject::find_satisfier(name, false))
                    .is_some()
                {
                    dep.push_str(INSTALLED_LABEL);
                }

                dep
            })
            .collect()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Add property rows
        self.add_property(PropID::Name, PropType::Title);
        self.add_property(PropID::Version, PropType::Text);
        self.add_property(PropID::Description, PropType::Text);
        self.add_property(PropID::PackageUrl, PropType::Link);
        self.add_property(PropID::Url, PropType::Link);
        self.add_property(PropID::Status, PropType::Text);
        self.add_property(PropID::Repository, PropType::Text);
        self.add_property(PropID::Groups, PropType::Text);
        self.add_property(PropID::Dependencies, PropType::LinkList);
        self.add_property(PropID::Optional, PropType::LinkList);
        self.add_property(PropID::Make, PropType::LinkList);
        self.add_property(PropID::RequiredBy, PropType::LinkList);
        self.add_property(PropID::OptionalFor, PropType::LinkList);
        self.add_property(PropID::Provides, PropType::Text);
        self.add_property(PropID::ConflictsWith, PropType::LinkList);
        self.add_property(PropID::Replaces, PropType::LinkList);
        self.add_property(PropID::Licenses, PropType::Text);
        self.add_property(PropID::Architecture, PropType::Text);
        self.add_property(PropID::Packager, PropType::Packager);
        self.add_property(PropID::BuildDate, PropType::Text);
        self.add_property(PropID::InstallDate, PropType::Text);
        self.add_property(PropID::DownloadSize, PropType::Text);
        self.add_property(PropID::InstalledSize, PropType::Text);
        self.add_property(PropID::InstallScript, PropType::Text);
        self.add_property(PropID::SHA256Sum, PropType::Text);

        // Set files search entry key capture widget
        imp.files_search_entry.set_key_capture_widget(Some(&imp.files_view.get()));

        // Bind files count to files count label
        imp.files_selection.bind_property("n-items", &imp.files_count_label.get(), "label")
            .transform_to(move |_, n_items: u32| Some(n_items.to_string()))
            .sync_create()
            .build();

        // Bind files count to files open/copy button states
        imp.files_selection.bind_property("n-items", &imp.files_open_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        imp.files_selection.bind_property("n-items", &imp.files_copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        // Bind log count to log copy button state
        imp.log_selection.bind_property("n-items", &imp.log_copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        // Bind cache count to cache header label
        imp.cache_selection.bind_property("n-items", &imp.cache_count_label.get(), "label")
            .transform_to(move |_, n_items: u32| Some(n_items.to_string()))
            .sync_create()
            .build();

        // Bind cache count to cache open/copy button states
        imp.cache_selection.bind_property("n-items", &imp.cache_open_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        imp.cache_selection.bind_property("n-items", &imp.cache_copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();

        // Bind backup count to backup header label
        imp.backup_selection.bind_property("n-items", &imp.backup_count_label.get(), "label")
            .transform_to(move |_, n_items: u32| Some(n_items.to_string()))
            .sync_create()
            .build();

        // Bind selected backup item to backup open button state
        imp.backup_selection.bind_property("selected-item", &imp.backup_open_button.get(), "sensitive")
            .transform_to(|_, item: Option<glib::Object>| {
                item.and_downcast::<BackupObject>()
                    .map_or(Some(false), |object| {
                        let status = object.status();

                        Some(status != BackupStatus::Locked && status != BackupStatus::All)
                    })
            })
            .sync_create()
            .build();

        // Bind backup count to backup copy button state
        imp.backup_selection.bind_property("n-items", &imp.backup_copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .sync_create()
            .build();
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Info copy button clicked signal
        imp.info_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            #[weak] imp,
            move |_| {
                let copy_text = format!("## Package Information\n{body}",
                    body={
                        let mut properties: Vec<String> = vec![];

                        let mut child = imp.info_listbox.first_child();

                        while let Some(row) = child.and_downcast::<PropertyValue>() {
                            if !(row.label().is_empty() || row.value().is_empty()) {
                                properties.push(format!("- **{}** : {}", row.label(), row.value()));
                            }

                            child = row.next_sibling();
                        }

                        properties.join("\n")
                    }
                );


                infopane.clipboard().set_text(&copy_text);
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
                    .expect("Could not downcast to 'StringObject'");

                open_with_default_app(&item.string());
            }
        ));

        // Files copy button clicked signal
        imp.files_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            #[weak] imp,
            move |_| {
                let copy_text = format!("{header}\n{body}",
                    header=format_args!("## {}\n|Files|\n|---|",infopane.pkg().unwrap().name()),
                    body=imp.files_selection.iter::<glib::Object>().flatten()
                        .map(|item| {
                            item
                                .downcast::<gtk::StringObject>()
                                .expect("Could not downcast to 'StringObject'")
                                .string()
                        })
                        .join("\n")
                );

                infopane.clipboard().set_text(&copy_text);
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

        // Log copy button clicked signal
        imp.log_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            #[weak] imp,
            move |_| {
                let copy_text = format!("{header}\n{body}",
                    header=format_args!("## {}\n|Log Messages|\n|---|", infopane.pkg().unwrap().name()),
                    body=imp.log_model.iter::<gtk::StringObject>().flatten()
                        .map(|item| item.string())
                        .join("\n")
                );

                infopane.clipboard().set_text(&copy_text);
            }
        ));

        // Cache open button clicked signal
        imp.cache_open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.cache_selection.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .expect("Could not downcast to 'StringObject'");

                open_with_default_app(&item.string());
            }
        ));

        // Cache copy button clicked signal
        imp.cache_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            #[weak] imp,
            move |_| {
                let copy_text = format!("{header}\n{body}",
                    header=format_args!("## {}\n|Cache Files|\n|---|", infopane.pkg().unwrap().name()),
                    body=imp.cache_model.iter::<gtk::StringObject>().flatten()
                        .map(|item| item.string())
                        .join("\n")
                );

                infopane.clipboard().set_text(&copy_text);
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

        // Backup open button clicked signal
        imp.backup_open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.backup_selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Could not downcast to 'BackupObject'");

                open_with_default_app(&item.filename());
            }
        ));

        // Backup copy button clicked signal
        imp.backup_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            #[weak] imp,
            move |_| {
                let copy_text = format!("{header}\n{body}",
                    header=format_args!("## {}\n|Backup Files|Status|\n|---|---|", infopane.pkg().unwrap().name()),
                    body=imp.backup_model.iter::<BackupObject>().flatten()
                        .map(|item| format!("{}|{}", item.filename(), item.status_text()))
                        .join("\n")
                );

                infopane.clipboard().set_text(&copy_text);
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
    }

    //---------------------------------------
    // Display helper functions
    //---------------------------------------
    fn update_info_listbox(&self, pkg: &PkgObject) {
        // Name
        self.set_string_property(PropID::Name, true, &pkg.name(), None);

        // Version
        self.set_string_property(
            PropID::Version,
            true,
            &pkg.version(),
            if pkg.flags().intersects(PkgFlags::UPDATES) {Some("pkg-update")} else {None}
        );

        // Description
        self.set_string_property(PropID::Description, true, pkg.description(), None);

        // Package URL
        let package_url = pkg.package_url();

        self.set_string_property(PropID::PackageUrl, !package_url.is_empty(), &package_url, None);

        // URL
        self.set_string_property(PropID::Url, !pkg.url().is_empty(), pkg.url(), None);

        // Licenses
        self.set_string_property(PropID::Licenses, !pkg.licenses().is_empty(), pkg.licenses(), None);

        // Status
        let status_icon = pkg.status_icon();

        self.set_string_property(
            PropID::Status,
            true,
            &pkg.status(),
            if pkg.flags().intersects(PkgFlags::INSTALLED) {Some(&status_icon)} else {None}
        );

        // Repository
        self.set_string_property(PropID::Repository, true, &pkg.repository(), None);

        // Groups
        self.set_string_property(PropID::Groups, !pkg.groups().is_empty(), &pkg.groups(), None);

        // Depends
        self.set_vec_property(PropID::Dependencies, true, pkg.depends(), None);

        // Optdepends
        let optdepends = if pkg.flags().intersects(PkgFlags::INSTALLED) {
            Self::installed_optdeps(pkg.optdepends())
        } else {
            pkg.optdepends().to_vec()
        };

        self.set_vec_property(PropID::Optional, !optdepends.is_empty(), &optdepends, None);

        // Makedepends
        self.set_vec_property(PropID::Make, !pkg.makedepends().is_empty(), pkg.makedepends(), None);

        // Required by
        self.set_vec_property(PropID::RequiredBy, true, pkg.required_by(), None);

        // Optional for
        let optional_for = pkg.optional_for();

        self.set_vec_property(PropID::OptionalFor, !optional_for.is_empty(), optional_for, None);

        // Provides
        self.set_vec_property(PropID::Provides, !pkg.provides().is_empty(), pkg.provides(), None);

        // Conflicts
        self.set_vec_property(PropID::ConflictsWith, !pkg.conflicts().is_empty(), pkg.conflicts(), None);

        // Replaces
        self.set_vec_property(PropID::Replaces, !pkg.replaces().is_empty(), pkg.replaces(), None);

        // Architecture
        self.set_string_property(PropID::Architecture, !pkg.architecture().is_empty(), pkg.architecture(), None);

        // Packager
        self.set_string_property(PropID::Packager, true, pkg.packager(), None);

        // Build date
        self.set_string_property(PropID::BuildDate, pkg.build_date() != 0, &pkg.build_date_string(), None);

        // Install date
        self.set_string_property(PropID::InstallDate, pkg.install_date() != 0, &pkg.install_date_string(), None);

        // Download size
        self.set_string_property(PropID::DownloadSize, pkg.download_size() != 0, &pkg.download_size_string(), None);

        // Installed size
        self.set_string_property(PropID::InstalledSize, true, &pkg.install_size_string(), None);

        // Has script
        self.set_string_property(
            PropID::InstallScript,
            true,
            if pkg.has_script() {"Yes"} else {"No"},
            None
        );

        // SHA256 sum
        self.set_string_property(PropID::SHA256Sum, !pkg.sha256sum().is_empty(), pkg.sha256sum(), None);
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
        let log_lines: Vec<gtk::StringObject> = pkg.log().iter()
            .map(|line| gtk::StringObject::new(line))
            .collect();

        imp.log_model.splice(0, imp.log_model.n_items(), &log_lines);
    }

    fn update_cache_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Populate cache view
        let cache_list: Vec<gtk::StringObject> = pkg.cache().iter()
            .map(|cache_file| gtk::StringObject::new(cache_file))
            .collect();

        imp.cache_model.splice(0, imp.cache_model.n_items(), &cache_list);
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

        // Set main stack visible page
        let visible_stack_page = if self.pkg().is_some() {
            "properties"
        } else {
            "empty"
        };

        if imp.main_stack.visible_child_name().unwrap_or_default() != visible_stack_page {
            imp.main_stack.set_visible_child_name(visible_stack_page);
        }

        // Set tab switcher sensitivity
        let switcher_sensitive = self.pkg().is_some();

        if imp.tab_switcher.is_sensitive() != switcher_sensitive {
            imp.tab_switcher.set_sensitive(switcher_sensitive);
        }

        // Get package history
        let history = imp.pkg_history.borrow();

        // Set header prev/next button states
        let prev_sensitive = history.can_select_previous();

        if imp.prev_button.is_sensitive() != prev_sensitive {
            imp.prev_button.set_sensitive(prev_sensitive);
        }

        let next_sensitive = history.can_select_next();

        if imp.next_button.is_sensitive() != next_sensitive {
            imp.next_button.set_sensitive(next_sensitive);
        }

        // Clear files search entry
        imp.files_search_entry.set_text("");

        // If package is not none, display it
        if let Some(pkg) = self.pkg() {
            // Set header bar title
            let title = if history.len() > 1 {
                format!("{}/{}  |  {}", history.selected().unwrap_or_default() + 1, history.len(), pkg.name())
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
        if self.imp().pkg_history.borrow().select_previous() {
            self.update_display();
        }
    }

    pub fn display_next(&self) {
        if self.imp().pkg_history.borrow().select_next() {
            self.update_display();
        }
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
        Self::new()
    }
}
