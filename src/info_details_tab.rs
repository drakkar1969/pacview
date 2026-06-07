use std::cell::RefCell;
use std::collections::HashMap;
use std::borrow::Cow;
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::glib;
use glib::{closure_local, RustClosure};

use crate::{
    pkg_data::{PkgFlags, PkgValidation},
    pkg_object::PkgObject,
    info_row::{PropID, PropType, ValueType, InfoRow},
    text_widget::{INSTALLED_LABEL, TextWidget},
    source_window::SourceWindow,
    hash_window::HashWindow,
    utils::Paths,
};

//------------------------------------------------------------------------------
// MODULE: InfoDetailsTab
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoDetailsTab)]
    #[template(resource = "/com/github/PacView/ui/info_details_tab.ui")]
    pub struct InfoDetailsTab {
        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) desc_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) repo_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) size_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) version_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) update_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) pkgbuild_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) hashes_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) listbox: TemplateChild<gtk::ListBox>,

        #[property(get, set, nullable)]
        pkg: RefCell<Option<PkgObject>>,

        pub(super) info_row_map: RefCell<HashMap<PropID, InfoRow>>,
        pub(super) sel_widget: RefCell<Option<TextWidget>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoDetailsTab {
        const NAME: &'static str = "InfoDetailsTab";
        type Type = super::InfoDetailsTab;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            // Install actions
            Self::install_actions(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InfoDetailsTab {}
    impl WidgetImpl for InfoDetailsTab {}
    impl BoxImpl for InfoDetailsTab {}

    impl InfoDetailsTab {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Copy info action
            klass.install_action("infopane.copy-info", None, |pane, _, _| {
                if let Some(pkg) = pane.pkg() {
                    let mut output = String::from("## Package Information\n");

                    let _ = writeln!(output, "- **Name** : {}", pkg.name());
                    let _ = writeln!(output, "- **Version** : {}", pkg.version());
                    let _ = writeln!(output, "- **Description** : {}", pkg.description());
                    let _ = writeln!(output, "- **Repository** : {}", pkg.repository());
                    let _ = writeln!(output, "- **Installed Size** : {}", pkg.install_size_string());
                    let _ = writeln!(output, "- **Status** : {}", pkg.status());

                    let mut child = pane.imp().listbox.first_child();

                    while let Some(row) = child.and_downcast::<InfoRow>() {
                        if row.is_visible() {
                            let label = row.label();
                            let value = row.value();

                            if !(label.is_empty() || value.is_empty()) {
                                let _ = writeln!(output, "- **{label}** : {value}");
                            }
                        }

                        child = row.next_sibling();
                    }

                    pane.clipboard().set_text(&output);
                }
            });

            // Show PKGBUILD action
            klass.install_action("infopane.show-pkgbuild", None, |pane, _, _| {
                if let Some(pkg) = pane.pkg() {
                    let parent = pane.root()
                        .and_downcast::<gtk::Window>()
                        .expect("Failed to downcast to 'GtkWindow'");

                    let source_window = SourceWindow::new(&parent, &pkg);

                    source_window.present();
                }
            });

            // Show hashes action
            klass.install_action("infopane.show-hashes", None, |pane, _, _| {
                if let Some(pkg) = pane.pkg()
                    .filter(|pkg| {
                        let validation = pkg.validation();

                        !(validation.intersects(PkgValidation::UNKNOWN)
                            || validation.intersects(PkgValidation::NONE))
                    }) {
                        let parent = pane.root()
                            .and_downcast::<gtk::Window>()
                            .expect("Failed to downcast to 'GtkWindow'");

                        let hash_window = HashWindow::new(&parent, &pkg);

                        hash_window.present();
                    }
            });
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoDetailsTab
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoDetailsTab(ObjectSubclass<imp::InfoDetailsTab>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InfoDetailsTab {
    //---------------------------------------
    // Add info row function
    //---------------------------------------
    fn add_info_row(&self, id: PropID, ptype: PropType, pkg_link_handler: &RustClosure) {
        let imp = self.imp();

        let row = InfoRow::new(id, ptype);

        row.set_pkg_link_handler(pkg_link_handler.clone());

        row.connect_closure("selection-widget", false, closure_local!(
            #[weak] imp,
            move |_: InfoRow, widget: TextWidget| {
                if widget.has_selection() {
                    if imp.sel_widget.borrow().as_ref().is_none_or(|w| w != &widget)
                        && let Some(prev_widget) = imp.sel_widget.replace(Some(widget)) {
                            prev_widget.activate_action("text.select-none", None).unwrap();
                        }
                } else if imp.sel_widget.borrow().as_ref().is_some_and(|w| w == &widget) {
                    imp.sel_widget.replace(None);
                }
            }
        ));

        imp.listbox.append(&row);

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
                        .is_some_and(|(name, _)| PkgObject::has_local_satisfier(name)) {
                            dep.to_owned() + INSTALLED_LABEL
                        } else {
                            dep.to_owned()
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
    fn validation(flags: PkgValidation) -> String {
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
    // Update listbox function
    //---------------------------------------
    fn update_listbox(&self, pkg: &PkgObject) {
        // Popularity
        self.set_info_row(PropID::Popularity, ValueType::StrOpt(pkg.popularity()));

        // Out of Date
        self.set_info_row(PropID::OutOfDate, ValueType::StrOptNum(&pkg.out_of_date_string(), pkg.out_of_date()));

        // Package URL
        self.set_info_row(PropID::PackageUrl, ValueType::StrOpt(&pkg.package_url()));

        // URL
        self.set_info_row(PropID::Url, ValueType::StrOpt(pkg.url()));

        // Licenses
        self.set_info_row(PropID::Licenses, ValueType::VecOptJoin(pkg.licenses()));

        // Groups
        self.set_info_row(PropID::Groups, ValueType::VecOptJoin(pkg.groups()));

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
        self.set_info_row(PropID::BuildDate, ValueType::StrOptNum(&pkg.build_date_string(), pkg.build_date()));

        // Install date
        self.set_info_row(PropID::InstallDate, ValueType::StrOptNum(&pkg.install_date_string(), pkg.install_date()));

        // Download size
        self.set_info_row(PropID::DownloadSize, ValueType::StrOptNum(&pkg.download_size_string(), pkg.download_size()));

        // Has script
        self.set_info_row(PropID::InstallScript, ValueType::StrOpt(pkg.has_script()));

        // Validation
        self.set_info_row(PropID::Validation, ValueType::Str(&Self::validation(pkg.validation())));
    }

    //---------------------------------------
    // Update details function
    //---------------------------------------
    fn update_details(&self, pkg: &PkgObject, count_label: &str) {
        let imp = self.imp();

        // Update count label
        imp.count_label.set_label(count_label);

        // Show package information
        imp.name_label.set_label(&pkg.name());
        imp.desc_label.set_label(pkg.description());

        imp.status_label.set_css_classes(&pkg.status_css_classes());
        imp.status_label.set_label(pkg.status());

        imp.repo_label.set_label(&pkg.repository());
        imp.version_label.set_label(&pkg.version());
        imp.size_label.set_label(&pkg.install_size_string());

        if let Some(update) = pkg.update_version() {
            imp.update_label.set_visible(true);
            imp.update_label.set_label(&update);
        } else {
            imp.update_label.set_visible(false);
        }

        // Update button states
        imp.pkgbuild_button.set_visible(Paths::paru().is_ok());

        imp.hashes_button.set_visible({
            let validation = pkg.validation();

            !(validation.intersects(PkgValidation::UNKNOWN)
                || validation.intersects(PkgValidation::NONE))
        });
    }

    //---------------------------------------
    // Public setup details listbox function
    //---------------------------------------
    pub fn setup_details_listbox(&self, pkg_link_handler: RustClosure) {
        // Add info rows
        self.add_info_row(PropID::Popularity, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::OutOfDate, PropType::Error, &pkg_link_handler);
        self.add_info_row(PropID::PackageUrl, PropType::Link, &pkg_link_handler);
        self.add_info_row(PropID::Url, PropType::Link, &pkg_link_handler);
        self.add_info_row(PropID::Groups, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::Dependencies, PropType::LinkList, &pkg_link_handler);
        self.add_info_row(PropID::Optional, PropType::LinkList, &pkg_link_handler);
        self.add_info_row(PropID::Make, PropType::LinkList, &pkg_link_handler);
        self.add_info_row(PropID::RequiredBy, PropType::LinkList, &pkg_link_handler);
        self.add_info_row(PropID::OptionalFor, PropType::LinkList, &pkg_link_handler);
        self.add_info_row(PropID::Provides, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::ConflictsWith, PropType::LinkList, &pkg_link_handler);
        self.add_info_row(PropID::Replaces, PropType::LinkList, &pkg_link_handler);
        self.add_info_row(PropID::Licenses, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::Architecture, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::Packager, PropType::Packager, &pkg_link_handler);
        self.add_info_row(PropID::BuildDate, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::InstallDate, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::DownloadSize, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::InstallScript, PropType::Text, &pkg_link_handler);
        self.add_info_row(PropID::Validation, PropType::Text, &pkg_link_handler);
    }

    //---------------------------------------
    // Public update function
    //---------------------------------------
    pub fn update(&self, pkg: &PkgObject, count_label: &str) {
        self.update_listbox(pkg);
        self.update_details(pkg, count_label);

        self.set_pkg(Some(pkg));
    }
}
