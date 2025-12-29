use std::cell::RefCell;
use std::marker::PhantomData;
use std::collections::HashMap;
use std::borrow::Cow;
use std::fmt::Write as _;
use std::time::Duration;

use gtk::glib;
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::closure_local;
use glib::clone;

use crate::package_view::AUR_PKGS;
use crate::text_widget::{TextWidget, INSTALLED_LABEL};
use crate::info_files_tab::InfoFilesTab;
use crate::info_log_tab::InfoLogTab;
use crate::info_cache_tab::InfoCacheTab;
use crate::info_backup_tab::InfoBackupTab;
use crate::info_row::{PropID, PropType, ValueType, InfoRow};
use crate::history_list::HistoryList;
use crate::pkg_data::{PkgFlags, PkgValidation};
use crate::pkg_object::PkgObject;
use crate::source_window::SourceWindow;
use crate::hash_window::HashWindow;

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
        pub(super) info_pkgbuild_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) info_hashes_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) info_copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) files_tab: TemplateChild<InfoFilesTab>,
        #[template_child]
        pub(super) log_tab: TemplateChild<InfoLogTab>,
        #[template_child]
        pub(super) cache_tab: TemplateChild<InfoCacheTab>,
        #[template_child]
        pub(super) backup_tab: TemplateChild<InfoBackupTab>,

        #[property(get = Self::pkg, set = Self::set_pkg, nullable)]
        pkg: PhantomData<Option<PkgObject>>,

        pub(super) info_row_map: RefCell<HashMap<PropID, InfoRow>>,
        pub(super) selection_widget: RefCell<Option<TextWidget>>,

        pub(super) pkg_history: RefCell<HistoryList>,

        pub(super) update_delay_id: RefCell<Option<glib::SourceId>>,
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

            obj.setup_signals();
            obj.setup_widgets();
        }
    }

    impl WidgetImpl for InfoPane {}
    impl BinImpl for InfoPane {}
    impl InfoPane {
        //---------------------------------------
        // Property getter/setter
        //---------------------------------------
        fn pkg(&self) -> Option<PkgObject> {
            self.pkg_history.borrow().current_item()
        }

        fn set_pkg(&self, pkg: Option<PkgObject>) {
            self.main_stack.set_visible_child_name(
                if pkg.is_some() { "properties" } else { "empty" }
            );

            self.tab_switcher.set_sensitive(pkg.is_some());

            self.info_pkgbuild_button.set_sensitive(pkg.is_some());

            self.info_hashes_button.set_sensitive(
                pkg.as_ref().is_some_and(|pkg| {
                    let validation = pkg.validation();

                    !(validation.intersects(PkgValidation::UNKNOWN)
                        || validation.intersects(PkgValidation::NONE))
                })
            );

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
    // InfoRow pkg link handler
    //---------------------------------------
    fn pkg_link_handler(&self, pkg_name: &str, pkg_version: &str) {
        AUR_PKGS.with_borrow(|aur_pkgs| {
            // Find link package in pacman databases
            let pkg_link = pkg_name.to_owned() + pkg_version;

            let pkg = PkgObject::find_satisfier(&pkg_link);

            // Find link package in AUR search results
            let new_pkg = pkg
                .or_else(|| {
                    aur_pkgs.iter()
                        .find(|&pkg| pkg.name() == pkg_name)
                        .or_else(|| {
                            aur_pkgs.iter()
                                .find(|&pkg| pkg.provides().iter().any(|s| s == &pkg_link))
                        })
                        .cloned()
                });

            // If link package found
            if let Some(pkg) = new_pkg {
                let pkg_history = self.imp().pkg_history.borrow();

                // If link package is in infopane history, select it
                // Otherwise append it after current history package
                pkg_history.set_current_or_make_last(pkg);

                // Display link package
                self.update_display();
            }
        });
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

        // Info PKGBUILD button clicked signal
        imp.info_pkgbuild_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                infopane.show_pkgbuild();
            }
        ));

        // Info hashes button clicked signal
        imp.info_hashes_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                infopane.show_hashes();
            }
        ));

        // Info copy button clicked signal
        imp.info_copy_button.connect_clicked(clone!(
            #[weak(rename_to = infopane)] self,
            move |_| {
                let mut output = String::from("## Package Information\n");

                let mut child = infopane.imp().info_listbox.first_child();

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

                infopane.clipboard().set_text(&output);
            }
        ));
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

        // Bind history list properties to widgets
        let pkg_history = imp.pkg_history.borrow();

        pkg_history.bind_property("peek-previous", &imp.prev_button.get(), "sensitive")
            .sync_create()
            .build();

        pkg_history.bind_property("peek-next", &imp.next_button.get(), "sensitive")
            .sync_create()
            .build();
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

        row.connect_closure("selection-widget", false, closure_local!(
            #[weak] imp,
            move |_: InfoRow, widget: TextWidget| {
                if widget.has_selection() {
                    if imp.selection_widget.borrow().as_ref()
                        .is_none_or(|selection_widget| selection_widget != &widget)
                    {
                        if let Some(selection_widget) = imp.selection_widget.replace(Some(widget)) {
                            selection_widget.activate_action("text.select-none", None).unwrap();
                        }
                    }
                } else if imp.selection_widget.borrow().as_ref()
                    .is_some_and(|selection_widget| selection_widget == &widget)
                {
                    imp.selection_widget.replace(None);
                }
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
        self.set_info_row(PropID::OutOfDate, ValueType::StrOptNum(&pkg.out_of_date_string(), pkg.out_of_date()));

        // Package URL
        self.set_info_row(PropID::PackageUrl, ValueType::StrOpt(&pkg.package_url()));

        // URL
        self.set_info_row(PropID::Url, ValueType::StrOpt(pkg.url()));

        // Licenses
        self.set_info_row(PropID::Licenses, ValueType::VecOptJoin(pkg.licenses()));

        // Status
        let status_icon = pkg.status_icon();

        self.set_info_row(PropID::Status,
            ValueType::StrIcon(
                pkg.status(),
                pkg.flags().intersects(PkgFlags::INSTALLED).then_some(status_icon)
            )
        );

        // Repository
        self.set_info_row(PropID::Repository, ValueType::Str(&pkg.repository()));

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

        // Installed size
        self.set_info_row(PropID::InstalledSize, ValueType::Str(&pkg.install_size_string()));

        // Has script
        self.set_info_row(PropID::InstallScript, ValueType::StrOpt(pkg.has_script()));

        // Validation
        self.set_info_row(PropID::Validation, ValueType::Str(&Self::validation(pkg.validation())));
    }

    //---------------------------------------
    // Public display functions
    //---------------------------------------
    pub fn update_display(&self) {
        let imp = self.imp();

        // Clear header bar title
        imp.title_widget.set_title("");

        // If package is not none, display it
        if let Some(pkg) = self.pkg() {
            // Set header bar title
            let pkg_history = imp.pkg_history.borrow();

            let title = if pkg_history.len() > 1 {
                format!("{}  \u{2022}  {}/{}", pkg.name(), pkg_history.current() + 1, pkg_history.len())
            } else {
                pkg.name()
            };

            imp.title_widget.set_title(&title);

            // Populate info listbox
            self.update_info_listbox(&pkg);

            // Remove delay timer if present
            if let Some(delay_id) = imp.update_delay_id.take() {
                delay_id.remove();

                // Clear files/log/cache/backup views
                imp.files_tab.pause_view();
                imp.log_tab.pause_view();
                imp.cache_tab.pause_view();
                imp.backup_tab.pause_view();
            }

            // Start delay timer
            let delay_id = glib::timeout_add_local_once(
                Duration::from_millis(50),
                clone!(
                    #[weak] imp,
                    move || {
                        // Populate files/log/cache/backup views
                        imp.files_tab.update_view(&pkg);
                        imp.log_tab.update_view(&pkg);
                        imp.cache_tab.update_view(&pkg);
                        imp.backup_tab.update_view(&pkg);

                        imp.update_delay_id.take();
                    }
                )
            );

            imp.update_delay_id.replace(Some(delay_id));
        }
    }

    pub fn display_prev(&self) {
        self.imp().pkg_history.borrow().move_previous();

        self.update_display();
    }

    pub fn display_next(&self) {
        self.imp().pkg_history.borrow().move_next();

        self.update_display();
    }

    //---------------------------------------
    // Other public functions
    //---------------------------------------
    pub fn set_visible_tab(&self, tab: &str) {
        let imp = self.imp();

        if imp.tab_switcher.is_sensitive() {
            imp.tab_stack.set_visible_child_name(tab);
        }
    }

    pub fn show_pkgbuild(&self) {
        if let Some(pkg) = self.pkg() {
            let parent = self.root()
                .and_downcast::<gtk::Window>()
                .expect("Failed to downcast to 'GtkWindow'");

            let source_window = SourceWindow::new(&parent, &pkg);

            source_window.present();
        }
    }

    pub fn show_hashes(&self) {
        if let Some(pkg) = self.pkg()
            .filter(|pkg| {
                let validation = pkg.validation();

                !(validation.intersects(PkgValidation::UNKNOWN)
                    || validation.intersects(PkgValidation::NONE))
            }) {
                let parent = self.root()
                    .and_downcast::<gtk::Window>()
                    .expect("Failed to downcast to 'GtkWindow'");

                let hash_window = HashWindow::new(&parent, &pkg);

                hash_window.present();
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
