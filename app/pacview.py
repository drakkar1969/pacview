#!/usr/bin/env python

import gi, sys, os, urllib.parse, subprocess, shlex, re

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject, Pango

import pyalpm

from object_types import PkgStatus, PkgObject, PkgProperty

# Global path variable
app_dir = os.path.abspath(os.path.dirname(sys.argv[0]))

# Global gresource file
gresource = Gio.Resource.load(os.path.join(app_dir, "com.github.PacView.gresource"))
gresource._register()

#------------------------------------------------------------------------------
#-- CLASS: PKGDETAILSWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgdetailswindow.ui")
class PkgDetailsWindow(Adw.Window):
	__gtype_name__ = "PkgDetailsWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	pkg_label = Gtk.Template.Child()

	content_stack = Gtk.Template.Child()

	file_header_label = Gtk.Template.Child()
	files_model = Gtk.Template.Child()

	tree_label = Gtk.Template.Child()

	log_model = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	_pkg_object = None

	@GObject.Property(type=PkgObject, default=None)
	def pkg_object(self):
		return(self._pkg_object)

	@pkg_object.setter
	def pkg_object(self, value):
		self._pkg_object = value

		if value is not None:
			# Set package name
			self.pkg_label.set_text(value.name)

			# Populate file list
			self.file_header_label.set_text(f'Files ({len(value.files_list)})')
			self.files_model.splice(0, 0, value.files_list)

			# Populate dependency tree
			pkg_tree = subprocess.run(shlex.split(f'pactree{"" if (value.status_flags & PkgStatus.INSTALLED) else " -s"} {value.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			self.tree_label.set_label(re.sub(" provides.+", "", str(pkg_tree.stdout, 'utf-8')))

			# Populate log
			pkg_log = subprocess.run(shlex.split(f'paclog --no-color --package={value.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			log_lines = [re.sub("\[(.+)T(.+)\+.+\] (.+)", r"\1 \2 : \3", l) for l in str(pkg_log.stdout, 'utf-8').split('\n') if l != ""]

			self.log_model.splice(0, 0, log_lines)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Set tree label font
		self.tree_label.set_attributes(Pango.AttrList.from_string('0 -1 font-desc "Source Code Pro 11"'))

	#-----------------------------------
	# Button signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_button_toggled(self, button):
		if button.get_active() == True:
			page_id = button.get_child().get_last_child().get_label().lower()
			self.content_stack.set_visible_child_name(page_id)

#------------------------------------------------------------------------------
#-- CLASS: PKGINFOPANE
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkginfopane.ui")
class PkgInfoPane(Gtk.Overlay):
	__gtype_name__ = "PkgInfoPane"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	model = Gtk.Template.Child()

	prev_button = Gtk.Template.Child()
	next_button = Gtk.Template.Child()
	details_button = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	_pkg_list = []
	_pkg_index = -1

	@GObject.Property(type=PkgObject, default=None)
	def pkg_object(self):
		return(self._pkg_list[self._pkg_index] if (self._pkg_index >= 0 and self._pkg_index < len(self._pkg_list)) else None)

	@pkg_object.setter
	def pkg_object(self, value):
		self._pkg_list = [value]
		self._pkg_index = 0

		self.display_package(value)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_setup_value(self, factory, item):
		label = Gtk.Label(halign=Gtk.Align.START, wrap_mode=Pango.WrapMode.WORD, wrap=True, width_chars=30, max_width_chars=30, xalign=0, use_markup=True)
		label.connect("activate-link", self.on_link_activated)
		item.set_child(label)

	@Gtk.Template.Callback()
	def on_bind_value(self, factory, item):
		item.get_child().set_label(item.get_item().prop_value)

	#-----------------------------------
	# Link signal handler
	#-----------------------------------
	def on_link_activated(self, label, url):
		parse_url = urllib.parse.urlsplit(url)

		if parse_url.scheme != "pkg": return(False)

		pkg_name = parse_url.netloc

		pkg_dict = dict([(pkg.name, pkg) for pkg in app.pkg_objects])

		new_pkg = None

		if pkg_name in pkg_dict.keys():
			new_pkg = pkg_dict[pkg_name]
		else:
			for pkg in pkg_dict.values():
				if [s for s in pkg.provides_list if pkg_name in s] != []:
					new_pkg = pkg
					break

		if new_pkg is not None and new_pkg is not self._pkg_list[self._pkg_index]:
			self._pkg_list = self._pkg_list[:self._pkg_index+1]
			self._pkg_list.append(new_pkg)

			self._pkg_index += 1

			self.display_package(new_pkg)

		return(True)

	#-----------------------------------
	# Display functions
	#-----------------------------------
	def display_package(self, pkg_object):
		self.prev_button.set_sensitive(self._pkg_index > 0)
		self.next_button.set_sensitive(self._pkg_index < len(self._pkg_list) - 1)

		self.details_button.set_sensitive(pkg_object is not None)

		self.model.remove_all()

		if pkg_object is not None:
			self.model.append(PkgProperty("Name", f'<b>{pkg_object.name}</b>'))
			self.model.append(PkgProperty("Version", pkg_object.version))
			self.model.append(PkgProperty("Description", pkg_object.description))
			self.model.append(PkgProperty("URL", pkg_object.url))
			if pkg_object.repository in app.default_db_names: self.model.append(PkgProperty("Package URL", pkg_object.package_url))
			self.model.append(PkgProperty("Licenses", pkg_object.licenses))
			self.model.append(PkgProperty("Status", pkg_object.status if (pkg_object.status_flags & PkgStatus.INSTALLED) else "not installed"))
			self.model.append(PkgProperty("Repository", pkg_object.repository))
			if pkg_object.group != "":self.model.append(PkgProperty("Groups", pkg_object.group))
			if pkg_object.provides != "None": self.model.append(PkgProperty("Provides", pkg_object.provides))
			self.model.append(PkgProperty("Dependencies", pkg_object.depends))
			if pkg_object.optdepends != "None": self.model.append(PkgProperty("Optional", pkg_object.optdepends))
			self.model.append(PkgProperty("Required By", pkg_object.required_by))
			if pkg_object.optional_for != "None": self.model.append(PkgProperty("Optional For", pkg_object.optional_for))
			if pkg_object.conflicts != "None": self.model.append(PkgProperty("Conflicts With", pkg_object.conflicts))
			if pkg_object.replaces != "None": self.model.append(PkgProperty("Replaces", pkg_object.replaces))
			self.model.append(PkgProperty("Architecture", pkg_object.architecture))
			self.model.append(PkgProperty("Maintainer", pkg_object.maintainer))
			self.model.append(PkgProperty("Build Date", pkg_object.build_date_long))
			if pkg_object.install_date_long != "": self.model.append(PkgProperty("Install Date", pkg_object.install_date_long))
			if pkg_object.download_size != "": self.model.append(PkgProperty("Download Size", pkg_object.download_size))
			self.model.append(PkgProperty("Installed Size", pkg_object.install_size))
			self.model.append(PkgProperty("Install Script", pkg_object.install_script))

	def display_prev_package(self):
		if self._pkg_index > 0:
			self._pkg_index -=1

			self.display_package(self._pkg_list[self._pkg_index])

	def display_next_package(self):
		if self._pkg_index < len(self._pkg_list) - 1:
			self._pkg_index +=1

			self.display_package(self._pkg_list[self._pkg_index])

	#-----------------------------------
	# Details window function
	#-----------------------------------
	def show_package_details(self):
		if self._pkg_index >= 0 and self._pkg_index < len(self._pkg_list) and self._pkg_list[self._pkg_index] is not None:
			pkg_detailswindow = PkgDetailsWindow()
			pkg_detailswindow.set_transient_for(self.get_root())

			pkg_detailswindow.pkg_object = self._pkg_list[self._pkg_index]

			pkg_detailswindow.show()

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgcolumnview.ui")
class PkgColumnView(Gtk.Box):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	selection = Gtk.Template.Child()
	filter_model = Gtk.Template.Child()
	model = Gtk.Template.Child()

	repo_filter = Gtk.Template.Child()
	status_filter = Gtk.Template.Child()
	search_filter = Gtk.Template.Child()

	version_sorter = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	_current_status = PkgStatus.ALL
	_current_search = ""

	@GObject.Property(type=int, default=PkgStatus.ALL)
	def current_status(self):
		return(self._current_status)

	@current_status.setter
	def current_status(self, value):
		self._current_status = value

		self.status_filter.changed(Gtk.FilterChange.DIFFERENT)

	@GObject.Property(type=str, default="")
	def current_search(self):
		return(self._current_search)

	@current_search.setter
	def current_search(self, value):
		self._current_search = value

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	search_by_name = GObject.Property(type=bool, default=True)
	search_by_desc = GObject.Property(type=bool, default=False)
	search_by_group = GObject.Property(type=bool, default=False)
	search_by_deps = GObject.Property(type=bool, default=False)
	search_by_optdeps = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind column sorters to sort functions
		self.version_sorter.set_sort_func(self.sort_by_ver, "version")

		# Bind filters to filter functions
		self.status_filter.set_filter_func(self.filter_by_status)
		self.search_filter.set_filter_func(self.filter_by_search)

		# Sort view by name (first) column
		self.view.sort_by_column(self.view.get_columns()[0], Gtk.SortType.ASCENDING)

	#-----------------------------------
	# Sorter functions
	#-----------------------------------
	def sort_by_ver(self, item_a, item_b, prop):
		return(pyalpm.vercmp(item_a.get_property(prop), item_b.get_property(prop)))

	#-----------------------------------
	# Filter functions
	#-----------------------------------
	def filter_by_status(self, item):
		return(item.status_flags & self.current_status)

	def filter_by_search(self, item):
		if self.current_search == "":
			return(True)
		else:
			match_name = (self.current_search in item.name) if self.search_by_name else False
			match_desc = (self.current_search in item.description.lower()) if self.search_by_desc else False
			match_group = (self.current_search in item.group.lower()) if self.search_by_group else False
			match_deps = ([s for s in item.depends_list if self.current_search in s] != []) if self.search_by_deps else False
			match_optdeps = ([s for s in item.optdepends_list if self.current_search in s] != []) if self.search_by_optdeps else False

			return(match_name or match_desc or match_group or match_deps or match_optdeps)

#------------------------------------------------------------------------------
#-- CLASS: SIDEBARLISTBOXROW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/sidebarlistboxrow.ui")
class SidebarListBoxRow(Gtk.ListBoxRow):
	__gtype_name__ = "SidebarListBoxRow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	image = Gtk.Template.Child()
	label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	str_id = GObject.Property(type=str, default="")

	@GObject.Property(type=str)
	def icon_name(self):
		return(self.image.get_icon_name())

	@icon_name.setter
	def icon_name(self, value):
		self.image.set_from_icon_name(value)

	@GObject.Property(type=str)
	def label_text(self):
		return(self.label.get_text())

	@label_text.setter
	def label_text(self, value):
		self.label.set_text(value)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: MAINWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/mainwindow.ui")
class MainWindow(Adw.ApplicationWindow):
	__gtype_name__ = "MainWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	header_stack = Gtk.Template.Child()
	header_search_entry = Gtk.Template.Child()

	header_sidebar_btn = Gtk.Template.Child()
	header_infopane_btn = Gtk.Template.Child()
	header_search_btn = Gtk.Template.Child()

	repo_listbox = Gtk.Template.Child()
	repo_listbox_all = Gtk.Template.Child()

	status_listbox = Gtk.Template.Child()
	status_listbox_installed = Gtk.Template.Child()

	column_view = Gtk.Template.Child()
	info_pane = Gtk.Template.Child()

	count_label = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Connect header search entry to package column view
		self.header_search_entry.set_key_capture_widget(self.column_view)

		# Bind header search button state to search entry visibility
		self.header_search_btn.bind_property(
			"active",
			self.header_stack,
			"visible-child-name",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL,
			lambda binding, value: "search" if value == True else "title",
			lambda binding, value: (value == "search")
		)

		# Bind package column view selected item to info pane
		self.column_view.selection.bind_property(
			"selected-item",
			self.info_pane,
			"pkg_object",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		# Bind package column view count to status label text
		self.column_view.filter_model.bind_property(
			"n-items",
			self.count_label,
			"label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: f'{value} matching package{"s" if value != 1 else ""}'
		)

		# Add actions
		action_list = [
			( "toggle-sidebar", None, "", "true", self.toggle_sidebar_action ),
			( "toggle-infopane", None, "", "true", self.toggle_infopane_action ),
			( "search-start", self.search_start_action ),
			( "search-stop", self.search_stop_action ),
			( "search-toggle", None, "", "false", self.search_toggle_action ),
			( "search-by-name", None, "", "true", self.search_params_action ),
			( "search-by-desc", None, "", "false", self.search_params_action ),
			( "search-by-group", None, "", "false", self.search_params_action ),
			( "search-by-deps", None, "", "false", self.search_params_action ),
			( "search-by-optdeps", None, "", "false", self.search_params_action ),
			( "view-prev-package", self.view_prev_package_action ),
			( "view-next-package", self.view_next_package_action ),
			( "show-details-window", self.show_details_window_action ),
			( "refresh-dbs", self.refresh_dbs_action ),
			( "show-about", self.show_about_action ),
			( "quit-app", self.quit_app_action )
		]

		self.add_action_entries(action_list)

		# Add keyboard shortcuts
		app.set_accels_for_action("win.toggle-sidebar", ["<ctrl>b"])
		app.set_accels_for_action("win.toggle-infopane", ["<ctrl>i"])
		app.set_accels_for_action("win.search-start", ["<ctrl>f"])
		app.set_accels_for_action("win.search-stop", ["Escape"])
		app.set_accels_for_action("win.view-prev-package", ["<alt>Left"])
		app.set_accels_for_action("win.view-next-package", ["<alt>Right"])
		app.set_accels_for_action("win.show-details-window", ["Return", "KP_Enter"])
		app.set_accels_for_action("win.refresh-dbs", ["F5"])
		app.set_accels_for_action("win.show-about", ["F1"])
		app.set_accels_for_action("win.quit-app", ["<ctrl>q"])

		# Add items to package column view
		self.column_view.model.splice(0, len(self.column_view.model), app.pkg_objects)

		# Initialize sidebar listboxes
		self.init_sidebar()

		# Set initial focus on package column view
		self.set_focus(self.column_view.view)

	#-----------------------------------
	# Functions
	#-----------------------------------
	def init_sidebar(self):
		# Add rows to sidebar repository list box
		while(row := self.repo_listbox.get_row_at_index(1)):
			if row != self.repo_listbox_all: self.repo_listbox.remove(row)

		for db in app.db_names:
			self.repo_listbox.append(SidebarListBoxRow(icon_name="package-x-generic-symbolic", label_text=str.title(db), str_id=db))

		# Select initial repo/status
		self.repo_listbox.select_row(self.repo_listbox_all)
		self.status_listbox.select_row(self.status_listbox_installed)

	#-----------------------------------
	# Action handlers
	#-----------------------------------
	def toggle_sidebar_action(self, action, value, user_data):
		action.set_state(value)

		self.header_sidebar_btn.set_active(value)

	def toggle_infopane_action(self, action, value, user_data):
		action.set_state(value)

		self.header_infopane_btn.set_active(value)

	def search_start_action(self, action, value, user_data):
		self.header_search_entry.emit("search-started")

	def search_stop_action(self, action, value, user_data):
		self.header_search_entry.emit("stop-search")

	def search_toggle_action(self, action, value, user_data):
		action.set_state(value)

		if value.get_boolean():
			self.header_search_entry.emit("search-started")
		else:
			self.header_search_entry.emit("stop-search")

	def search_params_action(self, action, value, user_data):
		action.set_state(value)

		self.column_view.set_property(action.props.name, value)

		self.column_view.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	def view_prev_package_action(self, action, value, user_data):
		self.info_pane.display_prev_package()

	def view_next_package_action(self, action, value, user_data):
		self.info_pane.display_next_package()

	def show_details_window_action(self, action, value, user_data):
		self.info_pane.show_package_details()

	def refresh_dbs_action(self, action, value, user_data):
		app.populate_pkg_objects()

		self.column_view.model.splice(0, len(self.column_view.model), app.pkg_objects)

		self.init_sidebar()
		self.header_search_entry.emit("stop-search")

	def show_about_action(self, action, value, user_data):
		about_window = Adw.AboutWindow(
			application_name="PacView",
			application_icon="software-properties",
			developer_name="draKKar1969",
			version="1.0beta",
			website="https://github.com/drakkar1969/pacview",
			developers=["draKKar1969"],
			designers=["draKKar1969"],
			license_type=Gtk.License.GPL_3_0,
			transient_for=self)

		about_window.show()

	def quit_app_action(self, action, value, user_data):
		self.close()

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_repo_selected(self, listbox, row):
		if row is not None:
			self.column_view.repo_filter.set_search(row.str_id)

	@Gtk.Template.Callback()
	def on_status_selected(self, listbox, row):
		if row is not None:
			self.column_view.current_status = PkgStatus(int(row.str_id))

	@Gtk.Template.Callback()
	def on_search_started(self, entry):
		self.header_stack.set_visible_child_name("search")

		self.set_focus(self.header_search_entry)

	@Gtk.Template.Callback()
	def on_search_changed(self, entry):
		self.column_view.current_search = entry.get_text().lower()

	@Gtk.Template.Callback()
	def on_search_stopped(self, entry):
		entry.set_text("")

		self.header_stack.set_visible_child_name("title")

#------------------------------------------------------------------------------
#-- CLASS: LAUNCHERAPP
#------------------------------------------------------------------------------
class LauncherApp(Adw.Application):
	#-----------------------------------
	# Variables
	#-----------------------------------
	db_names = []
	pkg_objects = []

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, **kwargs):
		super().__init__(**kwargs)

		# Connect signal handlers
		self.connect("activate", self.on_activate)

		self.populate_pkg_objects()

	def populate_pkg_objects(self):
		# Path to pacman databases
		alpm_folder = "/var/lib/pacman"

		# Default database names
		self.default_db_names = ["core", "extra", "community", "multilib"]

		# Build list of configured database names
		dbs = subprocess.run(shlex.split(f'pacman-conf -l'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		self.db_names = [n for n in str(dbs.stdout, 'utf-8').split('\n') if n != ""]

		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", alpm_folder)

		# Clear list of PkgOBjects
		self.pkg_objects.clear()

		# Build dictionary of local packages
		local_db = alpm_handle.get_localdb()
		local_dict = dict([(pkg.name, pkg) for pkg in local_db.pkgcache])

		# Build list of PkgOBjects from packages in databases
		for db in self.db_names:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				self.pkg_objects.extend([PkgObject(pkg) for pkg in sync_db.pkgcache])

		# Set status for installed packages
		for i, obj in enumerate(self.pkg_objects):
			if obj.pkg.name in local_dict.keys():
				local_pkg = local_dict[obj.pkg.name]
				reason = local_pkg.reason

				self.pkg_objects[i].local_pkg = local_pkg

				if reason == 0: self.pkg_objects[i].status_flags = PkgStatus.EXPLICIT
				else:
					if reason == 1:
						if local_pkg.compute_requiredby() != []:
							self.pkg_objects[i].status_flags = PkgStatus.DEPENDENCY
						else:
							self.pkg_objects[i].status_flags = PkgStatus.OPTIONAL if local_pkg.compute_optionalfor() != [] else PkgStatus.ORPHAN

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	def on_activate(self, app):
		self.main_window = MainWindow(application=app)
		self.main_window.present()

#------------------------------------------------------------------------------
#-- MAIN APP
#------------------------------------------------------------------------------
app = LauncherApp(application_id="com.github.PacView")
app.run(sys.argv)
