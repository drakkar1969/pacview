use std::cell::{Cell, RefCell};
use std::sync::LazyLock;
use std::collections::{HashMap, HashSet};
use std::cmp::Ordering;
use std::fmt::Write as _;

use gtk::glib;
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, closure_local};

use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use raur::Raur;
use futures::future::join_all;

use crate::{
    package_item::PackageItem,
    pkg_data::{PkgFlags, PkgData},
    pkg_object::PkgObject,
    repo_item::{RepoItem, RepoItemState},
    search_bar::{SearchBar, SearchProp},
    info_pane::InfoPane,
    tokio_manager::TokioManager,
    utils::ListStoreFind,
};

//------------------------------------------------------------------------------
// ENUM: PackageViewState
//------------------------------------------------------------------------------
#[derive(Default, Debug, Clone, Copy)]
#[repr(u32)]
pub enum PackageViewState {
    #[default]
    Normal,
    PackageLoad,
    AURDownload,
    PkgbuildRepoFetch
}

//------------------------------------------------------------------------------
// ENUM: SortProp
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SortProp")]
pub enum SortProp {
    #[default]
    Name,
    Repository,
    Status,
    InstallDate,
    InstalledSize,
    Groups,
}

//------------------------------------------------------------------------------
// MODULE: PackageView
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PackageView)]
    #[template(resource = "/com/github/PacView/ui/package_view.ui")]
    pub struct PackageView {
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) grouping_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) sort_button: TemplateChild<adw::SplitButton>,

        #[template_child]
        pub(super) rootdir_banner: TemplateChild<adw::Banner>,

        #[property(get)]
        #[template_child]
        pub(super) search_bar: TemplateChild<SearchBar>,
        #[property(get)]
        #[template_child]
        pub(super) sidebar_button: TemplateChild<gtk::ToggleButton>,
        #[property(get)]
        #[template_child]
        pub(super) infopane_button: TemplateChild<gtk::ToggleButton>,
        #[property(get)]
        #[template_child]
        pub(super) main_menu_button: TemplateChild<gtk::MenuButton>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) loading_status: TemplateChild<adw::StatusPage>,

        #[property(get)]
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[property(get)]
        #[template_child]
        pub(super) view: TemplateChild<gtk::ListView>,
        #[property(get)]
        #[template_child]
        pub(super) pkg_model: TemplateChild<gio::ListStore>,
        #[property(get)]
        #[template_child]
        pub(super) aur_model: TemplateChild<gio::ListStore>,

        #[template_child]
        pub(super) sort_model: TemplateChild<gtk::SortListModel>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) repo_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) factory: TemplateChild<gtk::SignalListItemFactory>,
        #[template_child]
        pub(super) header_factory: TemplateChild<gtk::BuilderListItemFactory>,
        #[template_child]
        pub(super) sorter: TemplateChild<gtk::CustomSorter>,
        #[template_child]
        pub(super) section_sorter: TemplateChild<gtk::StringSorter>,

        #[property(get)]
        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) empty_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        aur_sidebar_item: RefCell<RepoItem>,
        #[property(get, set, construct)]
        info_pane: RefCell<InfoPane>,

        #[property(get, set, builder(SortProp::default()))]
        sort_prop: Cell<SortProp>,
        #[property(get, set, default = true, construct)]
        sort_ascending: Cell<bool>,
        #[property(get, set, default = false, construct)]
        grouping: Cell<bool>,

        #[property(get, set)]
        status_id: Cell<PkgFlags>,

        pub(super) search_term: RefCell<String>,
        pub(super) search_tokens: RefCell<Vec<String>>,

        pub(super) cancel_token: RefCell<Option<CancellationToken>>
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PackageView {
        const NAME: &'static str = "PackageView";
        type Type = super::PackageView;
        type ParentType = adw::Bin;

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
    impl ObjectImpl for PackageView {
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

    impl WidgetImpl for PackageView {}
    impl BinImpl for PackageView {}

    impl PackageView {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Sort prop property action
            klass.install_property_action("view.set-sort-prop", "sort-prop");

            // Sort ascending property action
            klass.install_property_action("view.set-sort-ascending", "sort-ascending");

            // Reset sort action
            klass.install_action("view.reset-sort", None, |view, _, _| {
                view.set_sort_prop(SortProp::default());
                view.set_sort_ascending(true);
            });

            // Grouping property action
            klass.install_property_action("view.set-grouping", "grouping");
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PackageView
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PackageView(ObjectSubclass<imp::PackageView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PackageView {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Factory setup signal
        imp.factory.connect_setup(|_, obj| {
            let list_item = obj
                .downcast_ref::<gtk::ListItem>()
                .expect("Failed to downcast to 'GtkLIstItem'");

            let package_item = PackageItem::default();

            package_item.setup(list_item);

            list_item.set_child(Some(&package_item));
        });

        // Factory bind signal
        imp.factory.connect_bind(|_, obj| {
            let list_item = obj
                .downcast_ref::<gtk::ListItem>()
                .expect("Failed to downcast to 'GtkListItem'");

            let package_item = list_item.child()
                .and_downcast::<PackageItem>()
                .expect("Failed to downcast to 'PackageItem'");

            let pkg = list_item.item()
                .and_downcast::<PkgObject>()
                .expect("Failed to downcast to 'PkgObject'");

            package_item.bind(&pkg);
        });

        // List view selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak(rename_to = view)] self,
            move |selection, _, _, _| {
                let imp = view.imp();

                let n_items = selection.n_items();

                imp.count_label.set_label(&format!(
                    "{n_items} matching package{}", if n_items == 1 { "" } else { "s" }
                ));

                imp.empty_status.set_visible(n_items == 0);
            }
        ));

        // List view selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(
            #[weak(rename_to = view)] self,
            move |selection| {
                let pkg = selection.selected_item()
                    .and_downcast::<PkgObject>();

                view.info_pane().set_pkg(pkg);
            }
        ));

        // List view activate signal
        imp.view.connect_activate(clone!(
            #[weak(rename_to = view)] self,
            move |_, index| {
                let pkg = view.imp().selection.item(index)
                    .and_downcast::<PkgObject>();

                view.info_pane().set_pkg(pkg);
            }
        ));

        // Sort prop property notify signal
        self.connect_sort_prop_notify(|view| {
            view.imp().sorter.changed(gtk::SorterChange::Different);
        });

        // Sort ascending property notify signal
        self.connect_sort_ascending_notify(|view| {
            let imp = view.imp();

            let sort_asc = view.sort_ascending();

            imp.sort_button.set_icon_name(
                if sort_asc {
                    "view-sort-ascending-symbolic"
                } else {
                    "view-sort-descending-symbolic"
                }
            );

            imp.sort_button.set_tooltip_text(
                Some(if sort_asc { "Sort Descending" } else { "Sort Ascending" })
            );

            imp.sorter.changed(gtk::SorterChange::Inverted);
        });

        // Grouping property notify signal
        self.connect_grouping_notify(|view| {
            let imp = view.imp();

            if view.grouping() {
                imp.view.set_header_factory(Some(&imp.header_factory.get()));
                imp.sort_model.set_section_sorter(Some(&imp.section_sorter.get()));
            } else {
                imp.view.set_header_factory(None::<&gtk::ListItemFactory>);
                imp.sort_model.set_section_sorter(None::<&gtk::Sorter>);
            }
        });

        // Search bar changed signal
        imp.search_bar.connect_closure("changed", false, closure_local!(
            #[weak] imp,
            move |bar: SearchBar| {
                let term = bar.text().trim().to_lowercase();

                let tokens: Vec<String> = term.split_whitespace()
                    .map(ToOwned::to_owned)
                    .collect();

                imp.search_term.replace(term);
                imp.search_tokens.replace(tokens);

                imp.search_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Search bar AUR Search signal
        imp.search_bar.connect_closure("aur-search", false, closure_local!(
            #[weak(rename_to = view)] self,
            move |search_bar: &SearchBar| {
                view.search_in_aur(search_bar);
            }
        ));

        // Search bar enabled property notify signal
        imp.search_bar.connect_enabled_notify(clone!(
            #[weak(rename_to = view)] self,
            move |bar| {
                if !bar.enabled() {
                    view.cancel_aur_search();

                    view.imp().view.grab_focus();
                }
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind search button state to search bar enabled state
        imp.search_button.bind_property("active", &imp.search_bar.get(), "enabled")
            .sync_create()
            .bidirectional()
            .build();

        // Set list view sorter function
        imp.sorter.set_sort_func(clone!(
            #[weak(rename_to = view)] self,
            #[upgrade_or] gtk::Ordering::Equal,
            move |item_a, item_b| {
                let pkg_a: &PkgObject = item_a
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let pkg_b: &PkgObject = item_b
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let sort = match view.sort_prop() {
                    SortProp::Name => pkg_a.name().partial_cmp(&pkg_b.name()),
                    SortProp::Repository => pkg_a.repository().partial_cmp(&pkg_b.repository()),
                    SortProp::Status => pkg_a.status().partial_cmp(pkg_b.status()),
                    SortProp::InstallDate => pkg_a.install_date().partial_cmp(&pkg_b.install_date()),
                    SortProp::InstalledSize => pkg_a.install_size().partial_cmp(&pkg_b.install_size()),
                    SortProp::Groups => pkg_a.groups().partial_cmp(pkg_b.groups()),
                }.unwrap_or(Ordering::Equal);

                if view.sort_ascending() {
                    sort
                } else {
                    sort.reverse()
                }.into()
            }
        ));

        // Set status filter function
        imp.status_filter.set_filter_func(clone!(
            #[weak(rename_to = view)] self,
            #[upgrade_or] false,
            move |item| {
                let pkg: &PkgObject = item
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                pkg.flags().intersects(view.status_id())
            }
        ));

        // Set search filter function
        imp.search_filter.set_filter_func(clone!(
            #[weak(rename_to = view)] self,
            #[upgrade_or] false,
            move |item| {
                let imp = view.imp();

                let search_term = imp.search_term.borrow();
                let search_tokens = imp.search_tokens.borrow();

                if search_term.is_empty() {
                    return true
                }

                let pkg = item
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let is_match = |prop: &str| -> bool {
                    if imp.search_bar.exact() {
                        prop.eq_ignore_ascii_case(&search_term)
                    } else {
                        search_tokens.iter().all(|token| {
                            prop.as_bytes()
                                .windows(token.len())
                                .any(|window| window.eq_ignore_ascii_case(token.as_bytes()))
                        })
                    }
                };

                match imp.search_bar.prop() {
                    SearchProp::Name => is_match(&pkg.name()),
                    SearchProp::NameDesc => is_match(&pkg.name()) || is_match(pkg.description().unwrap_or_default()),
                    SearchProp::Groups => pkg.groups().iter().any(|s| is_match(s)),
                    SearchProp::Deps => pkg.depends().iter().any(|s| is_match(s)),
                    SearchProp::Optdeps => pkg.optdepends().iter().any(|s| is_match(s)),
                    SearchProp::Provides => pkg.provides().iter().any(|s| is_match(s)),
                    SearchProp::Files => pkg.files().iter().any(|s| is_match(s)),
                }
            }
        ));

        // Set search bar key capture widget
        imp.search_bar.set_key_capture_widget(imp.view.upcast_ref());
    }

    //---------------------------------------
    // Public sidebar filter functions
    //---------------------------------------
    pub fn repo_filter_changed(&self, repo_id: Option<&str>) {
        self.imp().repo_filter.set_search(repo_id);
    }

    pub fn status_filter_changed(&self, status_id: PkgFlags) {
        self.set_status_id(status_id);

        self.imp().status_filter.changed(gtk::FilterChange::Different);
    }

    //---------------------------------------
    // Do search helper function
    //---------------------------------------
    async fn do_search(term: String, tokens: Vec<String>, prop: SearchProp) -> Result<Vec<PkgData>, raur::Error> {
        // Static AUR cache variable
        static AUR_CACHE: LazyLock<TokioMutex<raur::Cache>> = LazyLock::new(|| {
            TokioMutex::new(raur::Cache::default())
        });

        // Return if query arg too small
        if term.len() < 2 {
            return Err(raur::Error::Aur(String::from("Query arg too small.")))
        }

        // Return if attempting to search by files
        if prop == SearchProp::Files {
            return Err(raur::Error::Aur(String::from("Cannot search by files.")))
        }

        // Set search mode
        let search_by = match prop {
            SearchProp::Name => raur::SearchBy::Name,
            SearchProp::NameDesc => raur::SearchBy::NameDesc,
            SearchProp::Groups => raur::SearchBy::Groups,
            SearchProp::Deps => raur::SearchBy::Depends,
            SearchProp::Optdeps => raur::SearchBy::OptDepends,
            SearchProp::Provides => raur::SearchBy::Provides,
            SearchProp::Files => unreachable!(),
        };

        // Search for AUR packages
        let handle = raur::Handle::new();

        let search_results = join_all(tokens.iter().map(|t| handle.search_by(t, search_by)))
            .await
            .into_iter()
            .collect::<Result<Vec<Vec<raur::Package>>, raur::Error>>()?;

        // Get list of package names that match all search terms
        let search_names = search_results.split_first().map(|(first, rem)| {
            let sets: Vec<HashSet<&str>> = rem.iter()
                .map(|v| v.iter().map(|pkg| pkg.name.as_str()).collect())
                .collect();

            let search_names: Vec<&str> = first.iter()
                .map(|pkg| pkg.name.as_str())
                .filter(|&name| sets.iter().all(|set| set.contains(name)))
                .collect();

            search_names
        })
        .ok_or_else(|| raur::Error::Aur("failed to parse search results".into()))?;

        // Get AUR package info using cache
        let pkg_data = handle.cache_info(&mut *AUR_CACHE.lock().await, &search_names)
            .await?
            .iter()
            .map(|pkg| PkgData::from_aur(pkg))
            .collect();

        Ok(pkg_data)
    }

    //---------------------------------------
    // Reset AUR search function
    //---------------------------------------
    fn reset_aur_search(&self) {
        // Cancel ongoing AUR search if any
        self.cancel_aur_search();

        // Clear AUR search results
        self.imp().aur_model.remove_all();
    }

    //---------------------------------------
    // Cancel AUR search function
    //---------------------------------------
    fn cancel_aur_search(&self) {
        if let Some(token) = self.imp().cancel_token.take() {
            token.cancel();
        }

        self.aur_sidebar_item().set_state(RepoItemState::Reset);
    }

    //---------------------------------------
    // Search in AUR function
    //---------------------------------------
    fn search_in_aur(&self, search_bar: &SearchBar) {
        let imp = self.imp();

        let term = imp.search_term.borrow().to_owned();
        let tokens = imp.search_tokens.borrow().to_owned();
        let prop = search_bar.prop();

        // Reset AUR search
        self.reset_aur_search();

        // Return if search term is empty
        if term.is_empty() {
            return
        }

        // Show search spinner
        self.aur_sidebar_item().set_state(RepoItemState::Searching);

        // Search AUR
        glib::spawn_future_local(clone!(
            #[weak(rename_to = view)] self,
            #[weak] search_bar,
            async move {
                let imp = view.imp();

                // Spawn tokio task to search AUR
                let task = TokioManager::spawn(async move |_| {
                    Self::do_search(term, tokens, prop).await
                });

                // Store cancel token
                imp.cancel_token.replace(Some(task.cancel_token));

                // Await tasks
                let result = task.join_handle.await
                    .expect("Failed to complete tokio task")
                    .unwrap_or(Ok(vec![]));

                // Remove stored cancel token
                imp.cancel_token.replace(None);

                // Get AUR search results
                match result {
                    Ok(data_list) => {
                        if search_bar.enabled() {
                            let pkg_list: Vec<PkgObject> = data_list.into_iter()
                                .map(PkgObject::new)
                                .collect();

                            imp.aur_model.splice(0, imp.aur_model.n_items(), &pkg_list);
                        }

                        // Hide search spinner
                        view.aur_sidebar_item()
                            .set_state(RepoItemState::AurResults(imp.aur_model.n_items()));

                        search_bar.set_aur_status(Ok(()));
                    }
                    Err(error) => {
                        search_bar.set_aur_status(Err(error.to_string()));

                        // Hide search spinner
                        view.aur_sidebar_item().set_state(RepoItemState::Reset);
                    }
                }
            }
        ));
    }

    //---------------------------------------
    // Public set state functions
    //---------------------------------------
    pub fn set_state(&self, state: PackageViewState) {
        let imp = self.imp();

        match state {
            PackageViewState::Normal => {
                imp.stack.set_visible_child_name("view");
            }
            PackageViewState::PackageLoad => {
                imp.loading_status.set_title("Loading Pacman Databases");
                imp.stack.set_visible_child_name("spinner");
            }
            PackageViewState::AURDownload => {
                imp.loading_status.set_title("Downloading AUR Database");
                imp.stack.set_visible_child_name("spinner");
            }
            PackageViewState::PkgbuildRepoFetch => {
                imp.loading_status.set_title("Fetching PKGBUILD Repositories");
                imp.stack.set_visible_child_name("spinner");
            }
        }
    }

    //---------------------------------------
    // Public splice packages function
    //---------------------------------------
    pub fn splice_packages(&self, pkg_slice: &[PkgObject], clear: bool) {
        let imp = self.imp();

        let position = if clear { 0 } else { imp.pkg_model.n_items() };
        let removals = if clear { imp.pkg_model.n_items() } else { 0 };

        imp.pkg_model.splice(position, removals, pkg_slice);
    }

    //---------------------------------------
    // Public show updates function
    //---------------------------------------
    pub fn show_updates(&self, update_map: &HashMap<String, String>) {
        let imp = self.imp();

        for (name, version) in update_map {
            if let Some(pkg) = imp.pkg_model.find_with(|pkg: &PkgObject| &pkg.name() == name) {
                pkg.set_update_version(Some(version.to_owned()));
            }
        }
    }

    //---------------------------------------
    // Public copy list function
    //---------------------------------------
    pub fn copy_list(&self) {
        let mut output = String::from("## Package List\n|Package Name|Version|Repository|Status|Installed Size|Groups|\n|---|---|---|---|---:|---|\n");

        for pkg in self.imp().selection.iter::<glib::Object>()
            .filter_map(|item| item.ok().and_downcast::<PkgObject>()) {
                writeln!(output, "|{name}|{version}|{repo}|{status}|{size}|{groups}|",
                    name=pkg.name(),
                    version=pkg.version(),
                    repo=pkg.repository(),
                    status=pkg.status(),
                    size=pkg.install_size_string(),
                    groups=pkg.groups().join(" | ")
                )
                .unwrap();
            }

        self.clipboard().set_text(&output);
    }

    //---------------------------------------
    // Public set root dir indicator function
    //---------------------------------------
    pub fn set_root_dir_indicator(&self, root_dir: Option<&str>) {
        let imp = self.imp();

        imp.rootdir_banner.set_title(root_dir.unwrap_or_default());
        imp.rootdir_banner.set_revealed(root_dir.is_some());
    }
}

impl Default for PackageView {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
