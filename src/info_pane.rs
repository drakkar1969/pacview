use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::closure_local;

use url::Url;

use crate::text_layout::{TextLayout, PropType};
use crate::property_label::PropertyLabel;
use crate::property_value::PropertyValue;
use crate::pkg_object::{PkgObject, PkgFlags};

//------------------------------------------------------------------------------
// ENUM: PropID
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropID")]
pub enum PropID {
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

//------------------------------------------------------------------------------
// MODULE: InfoPane
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoPane)]
    #[template(resource = "/com/github/PacView/ui/info_pane.ui")]
    pub struct InfoPane {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub grid: TemplateChild<gtk::Grid>,

        #[template_child]
        pub overlay_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub overlay_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub overlay_prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub overlay_next_button: TemplateChild<gtk::Button>,

        #[property(get, set)]
        pkg_model: OnceCell<gtk::FlattenListModel>,

        #[property(name = "pkg", type = Option<PkgObject>, get = Self::pkg, set = Self::set_pkg, nullable)]
        pub history_selection: RefCell<gtk::SingleSelection>,

        pub property_map: RefCell<HashMap<PropID, (PropertyLabel, PropertyValue)>>,
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
            TextLayout::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
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

    impl InfoPane {
        //-----------------------------------
        // Custom pkg property getter/setter
        //-----------------------------------
        fn pkg(&self) -> Option<PkgObject> {
            self.history_selection.borrow().selected_item()
                .and_downcast::<PkgObject>()
        }

        fn set_pkg(&self, pkg: Option<&PkgObject>) {
            let hist_model = self.history_selection.borrow().model()
                .and_downcast::<gio::ListStore>()
                .expect("Could not downcast to 'ListStore'");

            hist_model.remove_all();

            if let Some(pkg) = pkg {
                hist_model.append(pkg);
            }

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
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Value label link handler
    //-----------------------------------
    fn link_handler(&self, link: &str) -> bool {
        if let Some(url) = Url::parse(link).ok().filter(|url| url.scheme() == "pkg") {
            if let Some(pkg_name) = url.domain() {
                let pkg_model = self.pkg_model();

                // Find link package by name
                let mut new_pkg = pkg_model.iter::<glib::Object>()
                    .flatten()
                    .map(|pkg| pkg.downcast::<PkgObject>().expect("Could not downcast to 'PkgObject'"))
                    .find(|pkg| pkg.name() == pkg_name);

                // If link package is none, find by provides
                if new_pkg.is_none() {
                    new_pkg = pkg_model.iter::<glib::Object>()
                        .flatten()
                        .map(|pkg| pkg.downcast::<PkgObject>().expect("Could not downcast to 'PkgObject'"))
                        .find(|pkg| {
                            pkg.provides().iter().any(|s| s.contains(pkg_name))
                        });
                }

                // If link package found
                if let Some(new_pkg) = new_pkg {
                    let hist_sel = self.imp().history_selection.borrow();

                    let hist_model = hist_sel.model()
                        .and_downcast::<gio::ListStore>()
                        .expect("Could not downcast to 'ListStore'");

                    // If link package is in infopane history, select it
                    if let Some(i) = hist_model.find(&new_pkg) {
                        hist_sel.set_selected(i);
                    } else {
                        // If link package is not in history, get current history package
                        let hist_index = hist_sel.selected();

                        // If history package is not the last one in history, truncate history list
                        if hist_index < hist_model.n_items() - 1 {
                            hist_model.splice(hist_index + 1, hist_model.n_items() - hist_index - 1, &Vec::<glib::Object>::new());
                        }

                        // Add link package to history
                        hist_model.append(&new_pkg);

                        // Update history selection to link package
                        hist_sel.set_selected(hist_index + 1);
                    }

                    // Display link package
                    self.update_display();
                }
            }

            // Link handled
            return true
        }

        // Link not handled
        false
    }

    //-----------------------------------
    // Add property function
    //-----------------------------------
    fn add_property(&self, id: PropID, ptype: PropType) {
        let imp = self.imp();

        let value = id.to_value();

        let (_, enum_value) = glib::EnumValue::from_value(&value)
            .expect("Could not create 'EnumValue'");

        let property_label = PropertyLabel::new(enum_value.name());

        imp.grid.attach(&property_label, 0, enum_value.value(), 1, 1);

        let property_value = PropertyValue::new(ptype, closure_local!(@watch self as infopane => move |_: TextLayout, link: String| -> bool {
            infopane.link_handler(&link)
        }));

        imp.grid.attach(&property_value, 1, enum_value.value(), 1, 1);

        imp.property_map.borrow_mut().insert(id, (property_label, property_value));
    }

    //-----------------------------------
    // Public set property value function
    //-----------------------------------
    pub fn set_property_value(&self, id: PropID, visible: bool, value: &str, icon: Option<&str>) {
        if let Some((property_label, property_value)) = self.imp().property_map.borrow().get(&id) {
            property_label.set_visible(visible);
            property_value.set_visible(visible);

            if visible {
                property_value.set_icon(icon);
                property_value.set_text(value);
            }
        }
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

        // If package is not none, display it
        if let Some(pkg) = self.pkg() {
            let hist_sel = imp.history_selection.borrow();

            let overlay_visible = hist_sel.n_items() > 1;

            // Set infopane toolbar previous/next box visibility
            if imp.overlay_box.is_visible() != overlay_visible {
                imp.overlay_box.set_visible(overlay_visible);
            }

            // Set infopane toolbar label
            if overlay_visible {
                imp.overlay_label.set_label(&format!("{}/{}", hist_sel.selected() + 1, hist_sel.n_items()));
            }

            // Set infopane toolbar prev/next button states
            let prev_sensitive = hist_sel.selected() > 0;

            if imp.overlay_prev_button.is_sensitive() != prev_sensitive {
                imp.overlay_prev_button.set_sensitive(prev_sensitive);
            }

            let next_sensitive = hist_sel.selected() + 1 < hist_sel.n_items();

            if imp.overlay_next_button.is_sensitive() != next_sensitive {
                imp.overlay_next_button.set_sensitive(next_sensitive);
            }

            // Name
            self.set_property_value(PropID::Name, true, &pkg.name(), None);
            // Version
            self.set_property_value(PropID::Version, true, &pkg.version(), if pkg.has_update() {Some("pkg-update")} else {None});
            // Description
            self.set_property_value(PropID::Description, true, &pkg.description(), None);
            // Package URL
            let package_url = pkg.package_url();
            self.set_property_value(PropID::PackageUrl, !package_url.is_empty(), &package_url, None);
            // URL
            self.set_property_value(PropID::Url, !pkg.url().is_empty(), &pkg.url(), None);
            // Licenses
            self.set_property_value(PropID::Licenses, !pkg.licenses().is_empty(), &pkg.licenses(), None);
            // Status
            let status = &pkg.status();
            let status_icon = pkg.status_icon();
            self.set_property_value(PropID::Status, true, if pkg.flags().intersects(PkgFlags::INSTALLED) {status} else {"not installed"}, if pkg.flags().intersects(PkgFlags::INSTALLED) {Some(&status_icon)} else {None});
            // Repository
            self.set_property_value(PropID::Repository, true, &pkg.repository(), None);
            // Groups
            self.set_property_value(PropID::Groups, !pkg.groups().is_empty(), &pkg.groups(), None);
            // Provides
            self.set_property_value(PropID::Provides, !pkg.provides().is_empty(), &pkg.provides().join("     "), None);
            // Depends
            self.set_property_value(PropID::Dependencies, true, &pkg.depends().join("     "), None);
            // Optdepends
            self.set_property_value(PropID::Optional, !pkg.optdepends().is_empty(), &pkg.optdepends().join("     "), None);
            // Makedepends
            self.set_property_value(PropID::Make, !pkg.makedepends().is_empty(), &pkg.makedepends().join("     "), None);
            // Required by
            self.set_property_value(PropID::RequiredBy, true, &pkg.required_by().join("     "), None);
            // Optional for
            let optional_for = pkg.optional_for();
            self.set_property_value(PropID::OptionalFor, !optional_for.is_empty(), &optional_for.join("     "), None);
            // Conflicts
            self.set_property_value(PropID::ConflictsWith, !pkg.conflicts().is_empty(), &pkg.conflicts().join("     "), None);
            // Replaces
            self.set_property_value(PropID::Replaces, !pkg.replaces().is_empty(), &pkg.replaces().join("     "), None);
            // Architecture
            self.set_property_value(PropID::Architecture, !pkg.architecture().is_empty(), &pkg.architecture(), None);
            // Packager
            self.set_property_value(PropID::Packager, true, &pkg.packager(), None);
            // Build date
            self.set_property_value(PropID::BuildDate, pkg.build_date() != 0, &pkg.build_date_long(), None);
            // Install date
            self.set_property_value(PropID::InstallDate, pkg.install_date() != 0, &pkg.install_date_long(), None);
            // Download size
            self.set_property_value(PropID::DownloadSize, pkg.download_size() != 0, &pkg.download_size_string(), None);
            // Installed size
            self.set_property_value(PropID::InstalledSize, true, &pkg.install_size_string(), None);
            // Has script
            self.set_property_value(PropID::InstallScript, true, if pkg.has_script() {"Yes"} else {"No"}, None);
            // SHA256 sum
            self.set_property_value(PropID::SHA256Sum, !pkg.sha256sum().is_empty(), &pkg.sha256sum(), None);
            // MD5 sum
            self.set_property_value(PropID::MD5Sum, !pkg.md5sum().is_empty(), &pkg.md5sum(), None);
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
}
