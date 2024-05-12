use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;

use raur::Raur;
use raur::ArcPackage;
use futures::future;

use crate::window::{AUR_SNAPSHOT, INSTALLED_PKG_NAMES};
use crate::pkg_object::{PkgData, PkgObject, PkgFlags};
use crate::search_header::{SearchHeader, SearchMode, SearchProp};
use crate::utils::tokio_runtime;

pub const DEFAULT_COLS: [&str; 6] = ["package", "version", "repository", "status", "date", "size"];
pub const DEFAULT_SORT_COL: &str = "package";

//------------------------------------------------------------------------------
// MODULE: PackageView
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PackageView)]
    #[template(resource = "/com/github/PacView/ui/package_view.ui")]
    pub struct PackageView {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) flatten_model: TemplateChild<gtk::FlattenListModel>,
        #[template_child]
        pub(super) pkg_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) aur_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) repo_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) empty_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) popover_menu: TemplateChild<gtk::PopoverMenu>,

        #[property(get, set, default = true, construct)]
        loading: Cell<bool>,
        #[property(get, set)]
        n_items: Cell<u32>,

        pub(super) aur_cache: RefCell<HashSet<ArcPackage>>
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PackageView {
        const NAME: &'static str = "PackageView";
        type Type = super::PackageView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BoxLayout>();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PackageView {
        //-----------------------------------
        // Custom signals
        //-----------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("selected")
                        .param_types([Option::<PkgObject>::static_type()])
                        .build(),
                    Signal::builder("activated")
                        .param_types([Option::<PkgObject>::static_type()])
                        .build(),
                ]
            })
        }

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_controllers();
            obj.setup_actions();
            obj.setup_signals();
        }

        //-----------------------------------
        // Destructor
        //-----------------------------------
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for PackageView {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PackageView
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PackageView(ObjectSubclass<imp::PackageView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PackageView {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind loading property to stack page
        self.bind_property("loading", &imp.stack.get(), "visible-child-name")
            .transform_to(|_, loading: bool| Some(if loading { "empty" } else { "view" }))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind item count to n_items property
        imp.filter_model.bind_property("n-items", self, "n-items")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind item count to empty label visibility
        imp.filter_model.bind_property("n-items", &imp.empty_label.get(), "visible")
            .transform_to(|_, n_items: u32| Some(n_items == 0))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Column view click gesture
        let gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();

        gesture.connect_pressed(clone!(@weak imp => move |_ , _, x, y| {
            let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

            imp.popover_menu.set_pointing_to(Some(&rect));
            imp.popover_menu.popup();
        }));

        self.add_controller(gesture);
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Add reset columns action
        let columns_action = gio::ActionEntry::builder("reset-columns")
            .activate(clone!(@weak self as view => move |_, _, _| {
                view.set_columns(&DEFAULT_COLS);
            }))
            .build();

        // Add actions to view action group
        let view_group = gio::SimpleActionGroup::new();

        self.insert_action_group("view", Some(&view_group));

        view_group.add_action_entries([columns_action]);

        // Add package view header menu property actions
        let columns = self.imp().view.columns();

        for col in columns.iter::<gtk::ColumnViewColumn>().flatten() {
            let col_action = gio::PropertyAction::new(&format!("show-column-{}", col.id().unwrap()), &col, "visible");

            view_group.add_action(&col_action);
        }
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Column view selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(@weak self as view => move |selection| {
            let selected_item = selection.selected_item()
                .and_downcast::<PkgObject>();

            view.emit_by_name::<()>("selected", &[&selected_item]);
        }));

        // Column view activate signal
        imp.view.connect_activate(clone!(@weak self as view => move |_, index| {
            let item = view.imp().selection.item(index)
                .and_downcast::<PkgObject>();

            view.emit_by_name::<()>("activated", &[&item]);
        }));
    }

    //-----------------------------------
    // Public filter functions
    //-----------------------------------
    pub fn set_repo_filter(&self, repo_id: Option<&str>) {
        self.imp().repo_filter.set_search(repo_id);
    }

    pub fn set_status_filter(&self, status_id: PkgFlags) {
        self.imp().status_filter.set_filter_func(move |item| {
            let pkg: &PkgObject = item
                .downcast_ref::<PkgObject>()
                .expect("Could not downcast to 'PkgObject'");

            pkg.flags().intersects(status_id)
        });
    }

    pub fn set_search_filter(&self, search_term: &str, mode: SearchMode, prop: SearchProp) {
        let imp = self.imp();

        if search_term.is_empty() {
            imp.search_filter.unset_filter_func();
        } else {
            let term = search_term.to_lowercase();

            imp.search_filter.set_filter_func(move |item| {
                let pkg: &PkgObject = item
                    .downcast_ref::<PkgObject>()
                    .expect("Could not downcast to 'PkgObject'");

                let search_fields = match prop {
                    SearchProp::Name => { vec![pkg.name()] },
                    SearchProp::NameDesc => { vec![pkg.name(), pkg.description()] },
                    SearchProp::Group => { vec![pkg.groups()] },
                    SearchProp::Deps => { pkg.depends() },
                    SearchProp::Optdeps => { pkg.optdepends() },
                    SearchProp::Provides => { pkg.provides() },
                    SearchProp::Files => { pkg.files() },
                };

                if mode == SearchMode::Exact {
                    search_fields.iter().any(|s| s.eq(&term))
                } else {
                    let mut results = term.split_whitespace()
                        .map(|t| {
                            search_fields.iter().any(|s| s.to_lowercase().contains(t))
                        });

                    if mode == SearchMode::All {
                        results.all(|x| x)
                    } else {
                        results.any(|x| x)
                    }
                }
            });
        }
    }

    //-----------------------------------
    // Public search in AUR function
    //-----------------------------------
    pub fn search_in_aur(&self, search_header: SearchHeader, search_term: &str, prop: SearchProp) {
        let imp = self.imp();

        let term = search_term.to_lowercase();

        search_header.set_spinning(true);

        INSTALLED_PKG_NAMES.with_borrow(|installed_pkg_names| {
            // Get AUR cache (need to clone for mutable reference)
            let mut aur_cache = imp.aur_cache.borrow_mut().clone();

            // Clear AUR search results
            imp.aur_model.remove_all();

            AUR_SNAPSHOT.replace(vec![]);

            glib::spawn_future_local(clone!(@weak imp, @strong installed_pkg_names => async move {
                // Spawn thread to search AUR
                let result = tokio_runtime().spawn(async move {
                    let handle = raur::Handle::new();

                    // Set search mode
                    if prop == SearchProp::Files {
                        return Err(raur::Error::Aur("Cannot search by files.".to_string()))
                    }

                    let search_by = match prop {
                        SearchProp::Name => { raur::SearchBy::Name },
                        SearchProp::NameDesc => { raur::SearchBy::NameDesc },
                        SearchProp::Group => { raur::SearchBy::Groups },
                        SearchProp::Deps => { raur::SearchBy::Depends },
                        SearchProp::Optdeps => { raur::SearchBy::OptDepends },
                        SearchProp::Provides => { raur::SearchBy::Provides },
                        SearchProp::Files => unreachable!(),
                    };

                    // Search for AUR packages
                    let search_results = future::join_all(term.split_whitespace()
                        .map(|t| handle.search_by(t, search_by))
                    )
                    .await;

                    let mut aur_names: HashSet<String> = HashSet::new();

                    for res in search_results {
                        if let Ok(aur_list) = res {
                            aur_names.extend(aur_list.iter().map(|pkg| pkg.name.to_string()))
                        } else {
                            return Err(res.unwrap_err())
                        }
                    }

                    // Get AUR package info using cache
                    let aur_list = handle.cache_info(&mut aur_cache, &aur_names.iter().collect::<Vec<&String>>()).await?;

                    let data_list: Vec<PkgData> = aur_list.into_iter()
                        .filter(|aurpkg| !installed_pkg_names.contains(&aurpkg.name))
                        .map(|aurpkg| {
                            PkgData::from_aur(&aurpkg)
                        })
                        .collect();

                    Ok::<(HashSet<ArcPackage>, Vec<PkgData>), raur::Error>((aur_cache, data_list))
                })
                .await
                .expect("Could not complete async task");

                // Process thread result
                match result {
                    Ok((aur_cache, data_list)) => {
                        if search_header.enabled() {
                            let pkg_list: Vec<PkgObject> = data_list.into_iter()
                                .map(|data| PkgObject::new(None, data))
                                .collect();

                            imp.aur_model.splice(0, imp.aur_model.n_items(), &pkg_list);

                            AUR_SNAPSHOT.replace(pkg_list);
                        }

                        imp.aur_cache.replace(aur_cache);

                        search_header.set_aur_error(false);
                        search_header.set_tooltip_text(None);
                    },
                    Err(error) => {
                        search_header.set_aur_error(true);
                        search_header.set_tooltip_text(Some(&format!("AUR Search Error ({})", error)));
                    }
                }

                search_header.set_spinning(false);
            }));
        });
    }

    //-----------------------------------
    // Public column functions
    //-----------------------------------
    pub fn set_columns(&self, column_ids: &[&str]) {
        let columns = self.imp().view.columns();

        // Iterate through column IDs
        for (i, id) in column_ids.iter().enumerate() {
            // If column exists with given ID, insert it at position
            if let Some(col) = columns.iter::<gtk::ColumnViewColumn>().flatten()
                .find(|col| col.id().unwrap() == *id)
            {
                self.imp().view.insert_column(i as u32, &col);
            }
        }

        // Show/hide columns
        for col in columns.iter::<gtk::ColumnViewColumn>().flatten() {
            col.set_visible(column_ids.contains(&col.id().unwrap().as_str()));
        }
    }

    pub fn columns(&self) -> Vec<String> {
        // Get visible column IDs
        self.imp().view.columns()
            .iter::<gtk::ColumnViewColumn>()
            .flatten()
            .filter_map(|col| if col.is_visible() {col.id().map(|s| s.to_string())} else {None})
            .collect()
    }

    pub fn set_sorting(&self, id: &str, ascending: bool) {
        // Find sort column by ID
        let col = self.imp().view.columns().iter::<gtk::ColumnViewColumn>()
            .flatten()
            .find(|col| col.id().unwrap() == *id);

        // Set sort column/order
        self.imp().view.sort_by_column(col.as_ref(), if ascending {gtk::SortType::Ascending} else {gtk::SortType::Descending});
    }

    pub fn sorting(&self) -> (String, bool) {
        // Get view sorter
        let sorter = self.imp().view.sorter()
            .and_downcast::<gtk::ColumnViewSorter>()
            .expect("Could not downcast to 'ColumnViewSorter'");

        // Get sort column ID
        let sort_col = sorter.primary_sort_column().map_or(
            String::from(DEFAULT_SORT_COL),
            |col| col.id().unwrap().to_string()
        );

        // Get sort order
        let sort_asc = sorter.primary_sort_order() == gtk::SortType::Ascending;

        (sort_col, sort_asc)
    }

    //-----------------------------------
    // Public view function
    //-----------------------------------
    pub fn view(&self) -> gtk::ColumnView {
        self.imp().view.get()
    }

    //-----------------------------------
    // Public package functions
    //-----------------------------------
    pub fn splice_packages(&self, pkg_slice: &[PkgObject]) {
        let imp = self.imp();

        imp.pkg_model.splice(0, imp.pkg_model.n_items(), pkg_slice);
    }

    pub fn update_packages(&self, update_map: &HashMap<String, String>) {
        self.imp().pkg_model.iter::<PkgObject>()
            .flatten()
            .filter(|pkg| update_map.contains_key(&pkg.name()))
            .for_each(|pkg| {
                pkg.set_version(update_map[&pkg.name()].to_string());

                pkg.set_flags(pkg.flags() | PkgFlags::UPDATES);

                pkg.set_has_update(true);
            });
    }

    //-----------------------------------
    // Public copy list function
    //-----------------------------------
    pub fn copy_list(&self) -> String {
        self.imp().selection.iter::<glib::Object>()
            .flatten()
            .map(|item| {
                let pkg = item
                    .downcast::<PkgObject>()
                    .expect("Could not downcast to 'PkgObject'");

                format!("{repo}/{name}-{version}", repo=pkg.repository(), name=pkg.name(), version=pkg.version())
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

impl Default for PackageView {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
