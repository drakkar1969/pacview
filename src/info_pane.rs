use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::borrow::Cow;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::closure_local;
use glib::clone;

use crate::package_view::AUR_PKGS;
use crate::text_widget::{TextWidget, PropType, INSTALLED_LABEL, LINK_SPACER};
use crate::property_value::{ValueType, PropertyValue};
use crate::history_list::HistoryList;
use crate::pkg_data::PkgFlags;
use crate::pkg_object::PkgObject;
use crate::backup_object::{BackupObject, BackupStatus};
use crate::enum_traits::EnumExt;
use crate::utils::app_info;

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
    #[enum_value(name = "Popularity")]
    Popularity,
    #[enum_value(name = "Out of Date")]
    OutOfDate,
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
        pub(super) tab_switcher: TemplateChild<adw::InlineViewSwitcher>,
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
        #[property(get, set)]
        property_line_spacing: Cell<f64>,

        pub(super) property_map: RefCell<HashMap<PropID, PropertyValue>>,

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
        // Custom property getters/setters
        //---------------------------------------
        fn pkg(&self) -> Option<PkgObject> {
            self.pkg_history.borrow().selected_item()
        }

        fn set_pkg(&self, pkg: Option<&PkgObject>) {
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

        // Add property rows
        self.add_property(PropID::Name, PropType::Title);
        self.add_property(PropID::Version, PropType::Text);
        self.add_property(PropID::Description, PropType::Text);
        self.add_property(PropID::Popularity, PropType::Text);
        self.add_property(PropID::OutOfDate, PropType::Error);
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

        // Bind pkg property to widgets
        self.bind_property("pkg", &imp.main_stack.get(), "visible-child-name")
            .transform_to(move |_, pkg: Option<PkgObject>|
                Some(if pkg.is_some() { "properties" } else { "empty" })
            )
            .sync_create()
            .build();

        self.bind_property("pkg", &imp.tab_switcher.get(), "sensitive")
            .transform_to(move |_, pkg: Option<PkgObject>| Some(pkg.is_some()))
            .sync_create()
            .build();

        // Bind history list properties to widgets
        let pkg_history = imp.pkg_history.borrow();

        pkg_history.bind_property("can-select-prev", &imp.prev_button.get(), "sensitive")
            .sync_create()
            .build();

        pkg_history.bind_property("can-select-next", &imp.next_button.get(), "sensitive")
            .sync_create()
            .build();

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
            .transform_to(|_, item: Option<glib::Object>|
                item.and_downcast::<BackupObject>()
                    .map_or(Some(false), |object| {
                        let status = object.status();

                        Some(status != BackupStatus::Locked && status != BackupStatus::All)
                    })
            )
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

        // Info copy button clicked signal
        imp.info_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            #[weak] imp,
            move |_| {
                let body = {
                    let mut properties: Vec<String> = vec![];

                    let mut child = imp.info_listbox.first_child();

                    while let Some(row) = child.and_downcast::<PropertyValue>() {
                        if !(row.label().is_empty() || row.value().is_empty()) {
                            properties.push(format!("- **{}** : {}", row.label(), row.value()));
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
            #[weak] imp,
            move |_| {
                let body = imp.files_selection.iter::<glib::Object>().flatten()
                    .map(|item|
                        item
                            .downcast::<gtk::StringObject>()
                            .expect("Failed to downcast to 'StringObject'")
                            .string()
                    )
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

        // Log copy button clicked signal
        imp.log_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            #[weak] imp,
            move |_| {
                let body = imp.log_model.iter::<gtk::StringObject>().flatten()
                    .map(|item| item.string())
                    .collect::<Vec<glib::GString>>()
                    .join("\n");

                infopane.clipboard().set_text(
                    &format!("## {}\n|Log Messages|\n|---|\n{body}", infopane.pkg().unwrap().name())
                );
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
            #[weak] imp,
            move |_| {
                let body = imp.cache_model.iter::<gtk::StringObject>().flatten()
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
            #[weak] imp,
            move |_| {
                let body = imp.backup_model.iter::<BackupObject>().flatten()
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

        self.bind_property("property-line-spacing", &property, "line-spacing")
            .sync_create()
            .build();

        imp.info_listbox.append(&property);

        imp.property_map.borrow_mut().insert(id, property);
    }

    //---------------------------------------
    // Set property function
    //---------------------------------------
    fn set_property(&self, id: PropID, value: ValueType) {
        if let Some(property) = self.imp().property_map.borrow().get(&id) {
            let visible = match value {
                ValueType::Str(_) | ValueType::StrIcon(_, _) | ValueType::Vec(_) => true,
                ValueType::StrOpt(s) => !s.is_empty(),
                ValueType::StrOptNum(_, i) => i != 0,
                ValueType::VecOpt(v) => !v.is_empty(),
            };

            property.set_visible(visible);

            if visible {
                match value {
                    ValueType::Str(s) | ValueType::StrOpt(s) | ValueType::StrOptNum(s, _) => {
                        property.set_value(s);
                    },
                    ValueType::StrIcon(s, icon) => {
                        property.set_value(s);
                        property.set_icon(icon);
                    }
                    ValueType::Vec(v) | ValueType::VecOpt(v) => {
                        property.set_value(v.join(LINK_SPACER));
                    }
                }
            }

            if id == PropID::Status {
                property.set_icon_css_class("error", property.icon().unwrap_or_default() == "pkg-orphan");
            }
        }
    }

    //---------------------------------------
    // Get installed optdeps function
    //---------------------------------------
    fn installed_optdeps(flags: PkgFlags, optdepends: &[String]) -> Cow<'_, [String]> {
        if !optdepends.is_empty() && flags.intersects(PkgFlags::INSTALLED) {
            optdepends.iter()
                .map(|dep|
                    if dep.split_once([':'])
                        .and_then(|(name, _)| PkgObject::has_local_satisfier(name))
                        .unwrap_or_default()
                    {
                        dep.to_string() + INSTALLED_LABEL
                    } else {
                        dep.to_string()
                    }
                )
                .collect()
        } else {
            Cow::Borrowed(optdepends)
        }
    }

    //---------------------------------------
    // Display helper functions
    //---------------------------------------
    fn update_info_listbox(&self, pkg: &PkgObject) {
        // Name
        self.set_property(PropID::Name, ValueType::Str(&pkg.name()));

        // Version
        self.set_property(PropID::Version,
            ValueType::StrIcon(
                &pkg.version(),
                pkg.flags().intersects(PkgFlags::UPDATES).then_some("pkg-update")
            )
        );

        // Description
        self.set_property(PropID::Description, ValueType::StrOpt(pkg.description()));

        // Popularity
        self.set_property(PropID::Popularity, ValueType::StrOpt(pkg.popularity()));

        // Out of Date
        self.set_property(PropID::OutOfDate, ValueType::StrOptNum(pkg.out_of_date_string(), pkg.out_of_date()));

        // Package URL
        self.set_property(PropID::PackageUrl, ValueType::StrOpt(pkg.package_url()));

        // URL
        self.set_property(PropID::Url, ValueType::StrOpt(pkg.url()));

        // Licenses
        self.set_property(PropID::Licenses, ValueType::StrOpt(pkg.licenses()));

        // Status
        let status_icon = pkg.status_icon();

        self.set_property(PropID::Status,
            ValueType::StrIcon(
                &pkg.status(),
                pkg.flags().intersects(PkgFlags::INSTALLED).then_some(&status_icon)
            )
        );

        // Repository
        self.set_property(PropID::Repository, ValueType::Str(&pkg.repository()));

        // Groups
        self.set_property(PropID::Groups, ValueType::StrOpt(&pkg.groups()));

        // Depends
        self.set_property(PropID::Dependencies, ValueType::Vec(pkg.depends()));

        // Optdepends
        self.set_property(PropID::Optional, ValueType::VecOpt(&Self::installed_optdeps(pkg.flags(), pkg.optdepends())));

        // Makedepends
        self.set_property(PropID::Make, ValueType::VecOpt(pkg.makedepends()));

        // Required by
        self.set_property(PropID::RequiredBy, ValueType::Vec(pkg.required_by()));

        // Optional for
        self.set_property(PropID::OptionalFor, ValueType::VecOpt(pkg.optional_for()));

        // Provides
        self.set_property(PropID::Provides, ValueType::VecOpt(pkg.provides()));

        // Conflicts
        self.set_property(PropID::ConflictsWith, ValueType::VecOpt(pkg.conflicts()));

        // Replaces
        self.set_property(PropID::Replaces, ValueType::VecOpt(pkg.replaces()));

        // Architecture
        self.set_property(PropID::Architecture, ValueType::StrOpt(pkg.architecture()));

        // Packager
        self.set_property(PropID::Packager, ValueType::Str(pkg.packager()));

        // Build date
        self.set_property(PropID::BuildDate, ValueType::StrOptNum(pkg.build_date_string(), pkg.build_date()));

        // Install date
        self.set_property(PropID::InstallDate, ValueType::StrOptNum(pkg.install_date_string(), pkg.install_date()));

        // Download size
        self.set_property(PropID::DownloadSize, ValueType::StrOptNum(pkg.download_size_string(), pkg.download_size()));

        // Installed size
        self.set_property(PropID::InstalledSize, ValueType::Str(&pkg.install_size_string()));

        // Has script
        self.set_property(PropID::InstallScript, ValueType::StrOpt(pkg.has_script()));

        // SHA256 sum
        self.set_property(PropID::SHA256Sum, ValueType::StrOpt(pkg.sha256sum()));
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
        pkg.log_async(clone!(
            #[weak] imp,
            move |log| {
                let log_lines: Vec<gtk::StringObject> = log.iter()
                    .map(|line| gtk::StringObject::new(line))
                    .collect();

                imp.log_model.splice(0, imp.log_model.n_items(), &log_lines);
            }
        ));
    }

    fn update_cache_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        // Populate cache view
        pkg.cache_async(clone!(
            #[weak] imp,
            move |cache| {
                let cache_list: Vec<gtk::StringObject> = cache.iter()
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
                format!("{}/{}  |  {}", pkg_history.selected() + 1, pkg_history.n_items(), pkg.name())
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
        Self::new()
    }
}
