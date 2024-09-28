use std::cell::RefCell;
use std::collections::HashMap;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::closure_local;

use crate::window::{AUR_SNAPSHOT, INSTALLED_PKG_NAMES, PKG_SNAPSHOT};
use crate::text_widget::{TextWidget, PropType};
use crate::property_value::PropertyValue;
use crate::pkg_object::{PkgObject, PkgFlags};
use crate::traits::EnumValueExt;

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
    #[enum_value(name = "Licenses")]
    Licenses,
    #[enum_value(name = "Status")]
    Status,
    #[enum_value(name = "Repository")]
    Repository,
    #[enum_value(name = "Groups")]
    Groups,
    #[enum_value(name = "Provides")]
    Provides,
    #[enum_value(name = "Dependencies ")]
    Dependencies,
    #[enum_value(name = "Optional")]
    Optional,
    #[enum_value(name = "Make")]
    Make,
    #[enum_value(name = "Required By")]
    RequiredBy,
    #[enum_value(name = "Optional For")]
    OptionalFor,
    #[enum_value(name = "Conflicts With")]
    ConflictsWith,
    #[enum_value(name = "Replaces")]
    Replaces,
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
    #[enum_value(name = "MD5 Sum")]
    MD5Sum,
}

impl EnumValueExt for PropID {}

//------------------------------------------------------------------------------
// MODULE: InfoPane
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/info_pane.ui")]
    pub struct InfoPane {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) listbox: TemplateChild<gtk::ListBox>,

        #[template_child]
        pub(super) prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) next_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) details_button: TemplateChild<gtk::Button>,

        pub(super) property_map: RefCell<HashMap<PropID, PropertyValue>>,

        pub(super) history_selection: RefCell<gtk::SingleSelection>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoPane {
        const NAME: &'static str = "InfoPane";
        type Type = super::InfoPane;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for InfoPane {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
        }
    }

    impl WidgetImpl for InfoPane {}
    impl BinImpl for InfoPane {}
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
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // PropertyValue pkg link handler
    //-----------------------------------
    fn pkg_link_handler(&self, pkg_name: &str) {
        PKG_SNAPSHOT.with_borrow(|pkg_snapshot| {
            AUR_SNAPSHOT.with_borrow(|aur_snapshot| {
                // Find link package by name
                let mut new_pkg = pkg_snapshot.iter().chain(aur_snapshot)
                    .find(|&pkg| pkg.name() == pkg_name);

                // If link package is none, find by provides
                if new_pkg.is_none() {
                    new_pkg = pkg_snapshot.iter().chain(aur_snapshot)
                        .find(|&pkg| pkg.provides().iter().any(|s| s.contains(pkg_name)));
                }

                // If link package found
                if let Some(new_pkg) = new_pkg {
                    let hist_sel = self.imp().history_selection.borrow();

                    let hist_model = hist_sel.model()
                        .and_downcast::<gio::ListStore>()
                        .expect("Could not downcast to 'ListStore'");

                    // If link package is in infopane history, select it
                    if let Some(i) = hist_model.find(new_pkg) {
                        hist_sel.set_selected(i);
                    } else {
                        // If link package is not in history, get current history package
                        let hist_index = hist_sel.selected();

                        // If history package is not the last one in history, truncate history list
                        if hist_index < hist_model.n_items() - 1 {
                            hist_model.splice(hist_index + 1, hist_model.n_items() - hist_index - 1, &Vec::<glib::Object>::new());
                        }

                        // Add link package to history
                        hist_model.append(new_pkg);

                        // Update history selection to link package
                        hist_sel.set_selected(hist_index + 1);
                    }

                    // Display link package
                    self.update_display();
                }
            });
        });
    }

    //-----------------------------------
    // Add property function
    //-----------------------------------
    fn add_property(&self, id: PropID, ptype: PropType) {
        let imp = self.imp();

        let property_value = PropertyValue::new(
            ptype,
            closure_local!(
                #[watch(rename_to = infopane)]
                self,
                move |_: TextWidget, pkg_name: &str| {
                    infopane.pkg_link_handler(pkg_name)
                }
            )
        );

        imp.listbox.append(&property_value);

        imp.property_map.borrow_mut().insert(id, property_value);
    }

    //-----------------------------------
    // Set property functions
    //-----------------------------------
    fn set_string_property(&self, id: PropID, visible: bool, value: &str, icon: Option<&str>) {
        if let Some(property_value) = self.imp().property_map.borrow().get(&id) {
            property_value.set_visible(visible);

            if visible {
                property_value.set_label(id.enum_value().name());
                property_value.set_icon(icon);
                property_value.set_text(value);
            }
        }
    }

    fn set_vec_property(&self, id: PropID, visible: bool, value: &[String], icon: Option<&str>) {
        self.set_string_property(id, visible, &value.join("     "), icon);
    }

    //-----------------------------------
    // Get installed optdeps function
    //-----------------------------------
    fn installed_optdeps(&self, optdepends: &[String]) -> Vec<String> {
        optdepends.iter()
            .map(|dep| {
                let mut dep = dep.to_string();
                
                INSTALLED_PKG_NAMES.with_borrow(|installed_pkg_names| {
                    if dep.split_once(['<', '>', '=', ':'])
                        .filter(|&(name, _)| installed_pkg_names.contains(name))
                        .is_some()
                    {
                        dep.push_str(" [INSTALLED]");
                    }
                });

                dep
            })
            .collect::<Vec<String>>()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Initialize history selection
        let history_model = gio::ListStore::new::<PkgObject>();

        imp.history_selection.replace(gtk::SingleSelection::new(Some(history_model)));

        // Add property rows
        self.add_property(PropID::Name, PropType::Title);
        self.add_property(PropID::Version, PropType::Text);
        self.add_property(PropID::Description, PropType::Text);
        self.add_property(PropID::PackageUrl, PropType::Link);
        self.add_property(PropID::Url, PropType::Link);
        self.add_property(PropID::Licenses, PropType::Text);
        self.add_property(PropID::Status, PropType::Text);
        self.add_property(PropID::Repository, PropType::Text);
        self.add_property(PropID::Groups, PropType::Text);
        self.add_property(PropID::Provides, PropType::Text);
        self.add_property(PropID::Dependencies, PropType::LinkList);
        self.add_property(PropID::Optional, PropType::LinkList);
        self.add_property(PropID::Make, PropType::LinkList);
        self.add_property(PropID::RequiredBy, PropType::LinkList);
        self.add_property(PropID::OptionalFor, PropType::LinkList);
        self.add_property(PropID::ConflictsWith, PropType::LinkList);
        self.add_property(PropID::Replaces, PropType::LinkList);
        self.add_property(PropID::Architecture, PropType::Text);
        self.add_property(PropID::Packager, PropType::Packager);
        self.add_property(PropID::BuildDate, PropType::Text);
        self.add_property(PropID::InstallDate, PropType::Text);
        self.add_property(PropID::DownloadSize, PropType::Text);
        self.add_property(PropID::InstalledSize, PropType::Text);
        self.add_property(PropID::InstallScript, PropType::Text);
        self.add_property(PropID::SHA256Sum, PropType::Text);
        self.add_property(PropID::MD5Sum, PropType::Text);
    }

    //-----------------------------------
    // Public display functions
    //-----------------------------------
    pub fn update_display(&self) {
        let imp = self.imp();

        // Set stack visible page
        imp.stack.set_visible_child_name(if self.pkg().is_some() {"properties"} else {"empty"});

        // Set header details button state
        let details_sensitive = self.pkg().is_some();

        if imp.details_button.is_sensitive() != details_sensitive {
            imp.details_button.set_sensitive(details_sensitive);
        }

        // If package is not none, display it
        if let Some(pkg) = self.pkg() {
            let hist_sel = imp.history_selection.borrow();

            // Set header prev/next button states
            let prev_sensitive = hist_sel.selected() > 0;

            if imp.prev_button.is_sensitive() != prev_sensitive {
                imp.prev_button.set_sensitive(prev_sensitive);
            }

            let next_sensitive = hist_sel.selected() + 1 < hist_sel.n_items();

            if imp.next_button.is_sensitive() != next_sensitive {
                imp.next_button.set_sensitive(next_sensitive);
            }

            // Name
            self.set_string_property(PropID::Name, true, &pkg.name(), None);
            // Version
            self.set_string_property(PropID::Version, true, &pkg.version(), if pkg.has_update() {Some("pkg-update")} else {None});
            // Description
            self.set_string_property(PropID::Description, true, &pkg.description(), None);
            // Package URL
            let package_url = pkg.package_url();
            self.set_string_property(PropID::PackageUrl, !package_url.is_empty(), &package_url, None);
            // URL
            self.set_string_property(PropID::Url, !pkg.url().is_empty(), &pkg.url(), None);
            // Licenses
            self.set_string_property(PropID::Licenses, !pkg.licenses().is_empty(), &pkg.licenses(), None);
            // Status
            let status = pkg.status();
            let status_icon = pkg.status_icon();
            self.set_string_property(PropID::Status, true, if pkg.flags().intersects(PkgFlags::INSTALLED) {&status} else {"not installed"}, if pkg.flags().intersects(PkgFlags::INSTALLED) {Some(&status_icon)} else {None});
            // Repository
            self.set_string_property(PropID::Repository, true, &pkg.repository(), None);
            // Groups
            self.set_string_property(PropID::Groups, !pkg.groups().is_empty(), &pkg.groups(), None);
            // Provides
            self.set_vec_property(PropID::Provides, !pkg.provides().is_empty(), &pkg.provides(), None);
            // Depends
            self.set_vec_property(PropID::Dependencies, true, &pkg.depends(), None);
            // Optdepends
            let optdepends = if pkg.flags().intersects(PkgFlags::INSTALLED) {
                self.installed_optdeps(&pkg.optdepends())
            } else {
                pkg.optdepends()
            };
            self.set_vec_property(PropID::Optional, !optdepends.is_empty(), &optdepends, None);
            // Makedepends
            self.set_vec_property(PropID::Make, !pkg.makedepends().is_empty(), &pkg.makedepends(), None);
            // Required by
            self.set_vec_property(PropID::RequiredBy, true, &pkg.required_by(), None);
            // Optional for
            let optional_for = pkg.optional_for();
            self.set_vec_property(PropID::OptionalFor, !optional_for.is_empty(), &optional_for, None);
            // Conflicts
            self.set_vec_property(PropID::ConflictsWith, !pkg.conflicts().is_empty(), &pkg.conflicts(), None);
            // Replaces
            self.set_vec_property(PropID::Replaces, !pkg.replaces().is_empty(), &pkg.replaces(), None);
            // Architecture
            self.set_string_property(PropID::Architecture, !pkg.architecture().is_empty(), &pkg.architecture(), None);
            // Packager
            self.set_string_property(PropID::Packager, true, &pkg.packager(), None);
            // Build date
            self.set_string_property(PropID::BuildDate, pkg.build_date() != 0, &pkg.build_date_long(), None);
            // Install date
            self.set_string_property(PropID::InstallDate, pkg.install_date() != 0, &pkg.install_date_long(), None);
            // Download size
            self.set_string_property(PropID::DownloadSize, pkg.download_size() != 0, &pkg.download_size_string(), None);
            // Installed size
            self.set_string_property(PropID::InstalledSize, true, &pkg.install_size_string(), None);
            // Has script
            self.set_string_property(PropID::InstallScript, true, if pkg.has_script() {"Yes"} else {"No"}, None);
            // SHA256 sum
            self.set_string_property(PropID::SHA256Sum, !pkg.sha256sum().is_empty(), &pkg.sha256sum(), None);
            // MD5 sum
            self.set_string_property(PropID::MD5Sum, !pkg.md5sum().is_empty(), &pkg.md5sum(), None);
        }
    }

    pub fn display_prev(&self) {
        let hist_sel = self.imp().history_selection.borrow();

        if hist_sel.selected() > 0 {
            hist_sel.set_selected(hist_sel.selected() - 1);

            if hist_sel.selected_item().is_some() {
                self.update_display();
            }
        }
    }

    pub fn display_next(&self) {
        let hist_sel = self.imp().history_selection.borrow();

        if hist_sel.selected() + 1 < hist_sel.n_items() {
            hist_sel.set_selected(hist_sel.selected() + 1);

            if hist_sel.selected_item().is_some() {
                self.update_display();
            }
        }
    }

    //-----------------------------------
    // Public get/set pkg functions
    //-----------------------------------
    pub fn pkg(&self) -> Option<PkgObject> {
        self.imp().history_selection.borrow().selected_item()
            .and_downcast::<PkgObject>()
    }

    pub fn set_pkg(&self, pkg: Option<&PkgObject>) {
        let hist_model = self.imp().history_selection.borrow().model()
            .and_downcast::<gio::ListStore>()
            .expect("Could not downcast to 'ListStore'");

        hist_model.remove_all();

        if let Some(pkg) = pkg {
            hist_model.append(pkg);
        }

        self.update_display();
    }
}

impl Default for InfoPane {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
