use std::cell::RefCell;
use std::collections::HashMap;
use std::borrow::Cow;
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::glib;
use glib::{clone, RustClosure};

use crate::{
    pkg_data::{PkgFlags, PkgValidation},
    pkg_object::PkgObject,
    info_row::{PropID, PropType, ValueType, InfoRow},
    text_widget::{INSTALLED_LABEL, LINK_SPACER},
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
        pub(super) selection_row: RefCell<Option<InfoRow>>,
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
            klass.install_action("info.details-copy", None, |tab, _, _| {
                if let Some(pkg) = tab.pkg() {
                    let mut output = String::from("## Package Information\n");

                    let _ = writeln!(output, "- **Name** : {}", pkg.name());
                    let _ = writeln!(output, "- **Version** : {}", pkg.version());
                    let _ = writeln!(output, "- **Description** : {}", pkg.description());
                    let _ = writeln!(output, "- **Repository** : {}", pkg.repository());
                    let _ = writeln!(output, "- **Installed Size** : {}", pkg.install_size_string());
                    let _ = writeln!(output, "- **Status** : {}", pkg.status());

                    let mut child = tab.imp().listbox.first_child();

                    while let Some(row) = child.and_downcast::<InfoRow>() {
                        if row.is_visible() {
                            let label = row.label();
                            let value = row.value().replace(LINK_SPACER, " ");

                            if !(label.is_empty() || value.is_empty()) {
                                let _ = writeln!(output, "- **{label}** : {value}");
                            }
                        }

                        child = row.next_sibling();
                    }

                    tab.clipboard().set_text(&output);
                }
            });

            // Show PKGBUILD action
            klass.install_action("info.show-pkgbuild", None, |tab, _, _| {
                if let Some(pkg) = tab.pkg() {
                    let parent = tab.root()
                        .and_downcast::<gtk::Window>()
                        .expect("Failed to downcast to 'GtkWindow'");

                    let source_window = SourceWindow::new(&parent, &pkg);

                    source_window.present();
                }
            });

            // Show hashes action
            klass.install_action("info.show-hashes", None, |tab, _, _| {
                if let Some(pkg) = tab.pkg()
                    .filter(|pkg| {
                        let validation = pkg.validation();

                        !(validation.intersects(PkgValidation::UNKNOWN)
                            || validation.intersects(PkgValidation::NONE))
                    }) {
                        let parent = tab.root()
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
    // Public setup details function
    //---------------------------------------
    pub fn setup_details(&self, pkg_link_handler: RustClosure) {
        // Add info rows
        for (id, ptype) in [
            (PropID::Popularity, PropType::Text),
            (PropID::OutOfDate, PropType::Error),
            (PropID::PackageUrl, PropType::Link),
            (PropID::Url, PropType::Link),
            (PropID::Groups, PropType::Text),
            (PropID::Provides, PropType::Text),
            (PropID::Dependencies, PropType::LinkList),
            (PropID::Optional, PropType::LinkList),
            (PropID::Make, PropType::LinkList),
            (PropID::RequiredBy, PropType::LinkList),
            (PropID::OptionalFor, PropType::LinkList),
            (PropID::ConflictsWith, PropType::LinkList),
            (PropID::Replaces, PropType::LinkList),
            (PropID::Architecture, PropType::Text),
            (PropID::Licenses, PropType::Text),
            (PropID::Packager, PropType::Packager),
            (PropID::BuildDate, PropType::Text),
            (PropID::InstallDate, PropType::Text),
            (PropID::DownloadSize, PropType::Text),
            (PropID::InstallScript, PropType::Text),
            (PropID::Validation, PropType::Text)
        ] {
            let handler = [PropType::Link, PropType::LinkList, PropType::Packager]
                .contains(&ptype)
                .then_some(pkg_link_handler.clone());

            self.add_info_row(id, ptype, handler);

        }
    }

    //---------------------------------------
    // Add info row function
    //---------------------------------------
    fn add_info_row(&self, id: PropID, ptype: PropType, link_handler: Option<RustClosure>) {
        let imp = self.imp();

        let row = InfoRow::new(id, ptype, link_handler);

        row.connect_has_selection_notify(clone!(
            #[weak] imp,
            move |row| {
                if row.has_selection() {
                    if imp.selection_row.borrow().as_ref().is_none_or(|sel| sel != row) {
                        if let Some(prev_row) = imp.selection_row.replace(Some(row.clone())) {
                            prev_row.activate_action("text.select-none", None).unwrap();
                        }
                    }
                } else if imp.selection_row.borrow().as_ref().is_some_and(|sel| sel == row) {
                    imp.selection_row.replace(None);
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

        // Groups
        self.set_info_row(PropID::Groups, ValueType::VecOptJoin(pkg.groups()));

        // Provides
        self.set_info_row(PropID::Provides, ValueType::VecOpt(pkg.provides()));

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

        // Conflicts
        self.set_info_row(PropID::ConflictsWith, ValueType::VecOpt(pkg.conflicts()));

        // Replaces
        self.set_info_row(PropID::Replaces, ValueType::VecOpt(pkg.replaces()));

        // Architecture
        self.set_info_row(PropID::Architecture, ValueType::StrOpt(pkg.architecture()));

        // Licenses
        self.set_info_row(PropID::Licenses, ValueType::VecOptJoin(pkg.licenses()));

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
        self.set_info_row(PropID::Validation, ValueType::Str(&pkg.validation().to_string()));
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
    // Public update function
    //---------------------------------------
    pub fn update(&self, pkg: &PkgObject, count_label: &str) {
        self.update_listbox(pkg);
        self.update_details(pkg, count_label);

        self.set_pkg(Some(pkg));
    }
}
