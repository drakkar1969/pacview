#!/usr/bin/env python

import gi, sys, os, urllib.parse, subprocess, shlex, re

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject, Pango, Gdk

import pyalpm

from object_types import PkgStatus, PkgObject, PkgProperty, StatsItem

# Global path variable
app_dir = os.path.abspath(os.path.dirname(sys.argv[0]))

# Global gresource file
gresource = Gio.Resource.load(os.path.join(app_dir, "com.github.PacView.gresource"))
gresource._register()

#------------------------------------------------------------------------------
#-- CLASS: STATSWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/statswindow.ui")
class StatsWindow(Adw.Window):
	__gtype_name__ = "StatsWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	model = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

	#-----------------------------------
	# Key press signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_key_pressed(self, keyval, keycode, user_data, state):
		if keycode == Gdk.KEY_Escape and state == 0: self.close()

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_setup_left(self, factory, item):
		label = Gtk.Label(xalign=0, use_markup=True)
		item.set_child(label)

	@Gtk.Template.Callback()
	def on_setup_right(self, factory, item):
		label = Gtk.Label(xalign=1, use_markup=True)
		item.set_child(label)

	@Gtk.Template.Callback()
	def on_bind_repository(self, factory, item):
		item.get_child().set_label(item.get_item().repository)

	@Gtk.Template.Callback()
	def on_bind_count(self, factory, item):
		item.get_child().set_label(item.get_item().count)

	@Gtk.Template.Callback()
	def on_bind_size(self, factory, item):
		item.get_child().set_label(item.get_item().size)

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

	cache_model = Gtk.Template.Child()

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
			self.pkg_label.set_text(f'{value.repository}/{value.name}')

			# Populate file list
			self.file_header_label.set_text(f'Files ({len(value.files_list)})')
			self.files_model.splice(0, 0, value.files_list)

			# Populate dependency tree
			pkg_tree = subprocess.run(shlex.split(f'pactree{"" if (value.status_flags & PkgStatus.INSTALLED) else " -s"} {value.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			self.tree_label.set_label(re.sub(" provides.+", "", str(pkg_tree.stdout, 'utf-8')))

			# Populate log
			pkg_log = subprocess.run(shlex.split(f'paclog --no-color --package={value.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			log_lines = [re.sub("\[(.+)T(.+)\+.+\] (.+)", r"\1 \2 : \3", l) for l in str(pkg_log.stdout, 'utf-8').split('\n') if l != ""]

			self.log_model.splice(0, 0, log_lines[::-1]) # Reverse list

			# Populate cache
			pkg_cache = subprocess.run(shlex.split(f'paccache -vdk0 {value.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			cache_lines = [l for l in str(pkg_cache.stdout, 'utf-8').split('\n') if (l != "" and l.startswith("==>") == False and l.endswith(".sig") == False)]

			self.cache_model.splice(0, 0, cache_lines)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Set tree label font
		self.tree_label.set_attributes(Pango.AttrList.from_string('0 -1 font-desc "Source Code Pro 11"'))

	#-----------------------------------
	# Key press signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_key_pressed(self, keyval, keycode, user_data, state):
		if keycode == Gdk.KEY_Escape and state == 0: self.close()

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

	overlay_toolbar = Gtk.Template.Child()
	prev_button = Gtk.Template.Child()
	next_button = Gtk.Template.Child()

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

		self.overlay_toolbar.set_visible(False)

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
		image = Gtk.Image()

		label = Gtk.Label(halign=Gtk.Align.START, wrap_mode=Pango.WrapMode.WORD, wrap=True, xalign=0, use_markup=True)
		label.connect("activate-link", self.on_link_activated)

		box = Gtk.Box(spacing=6)
		box.append(image)
		box.append(label)

		item.set_child(box)

	@Gtk.Template.Callback()
	def on_bind_value(self, factory, item):
		image = item.get_child().get_first_child()
		icon = item.get_item().prop_icon

		image.set_visible(icon is not None)
		image.set_from_resource(icon)

		item.get_child().get_last_child().set_label(item.get_item().prop_value)

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

			self.overlay_toolbar.set_visible(True)

		return(True)

	#-----------------------------------
	# Display functions
	#-----------------------------------
	def display_package(self, pkg_object):
		self.prev_button.set_sensitive(self._pkg_index > 0)
		self.next_button.set_sensitive(self._pkg_index < len(self._pkg_list) - 1)

		self.model.remove_all()

		if pkg_object is not None:
			self.model.append(PkgProperty("Name", f'<b>{pkg_object.name}</b>'))
			self.model.append(PkgProperty("Version", pkg_object.version))
			self.model.append(PkgProperty("Description", pkg_object.description))
			self.model.append(PkgProperty("URL", pkg_object.url))
			if pkg_object.repository in app.sync_db_names: self.model.append(PkgProperty("Package URL", pkg_object.package_url))
			if pkg_object.repository == "AUR": self.model.append(PkgProperty("AUR URL", pkg_object.package_url))
			self.model.append(PkgProperty("Licenses", pkg_object.licenses))
			self.model.append(PkgProperty("Status", pkg_object.status if (pkg_object.status_flags & PkgStatus.INSTALLED) else "not installed", pkg_object.status_icon))
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
			details_window = PkgDetailsWindow()
			details_window.set_transient_for(app.main_window)

			details_window.pkg_object = self._pkg_list[self._pkg_index]

			details_window.show()

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
	current_status = GObject.Property(type=int, default=PkgStatus.ALL)
	current_search = GObject.Property(type=str, default="")

	search_by_name = GObject.Property(type=bool, default=True)
	search_by_desc = GObject.Property(type=bool, default=False)
	search_by_group = GObject.Property(type=bool, default=False)
	search_by_deps = GObject.Property(type=bool, default=False)
	search_by_optdeps = GObject.Property(type=bool, default=False)
	search_by_provides = GObject.Property(type=bool, default=False)

	search_params = GObject.Property(type=GObject.TYPE_STRV, default=["name"])

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
			match_provides = ([s for s in item.provides_list if self.current_search in s] != []) if self.search_by_provides else False

			return(match_name or match_desc or match_group or match_deps or match_optdeps or match_provides)

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
	header_details_btn = Gtk.Template.Child()

	repo_listbox = Gtk.Template.Child()
	repo_listbox_all = Gtk.Template.Child()

	status_listbox = Gtk.Template.Child()
	status_listbox_installed = Gtk.Template.Child()

	column_view = Gtk.Template.Child()
	info_pane = Gtk.Template.Child()

	count_label = Gtk.Template.Child()
	search_params_label = Gtk.Template.Child()

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

		# Bind info pane package to details button enabled state
		self.info_pane.bind_property(
			"pkg_object",
			self.header_details_btn,
			"sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value is not None
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
			( "search-by-provides", None, "", "false", self.search_params_action ),
			( "view-prev-package", self.view_prev_package_action ),
			( "view-next-package", self.view_next_package_action ),
			( "show-details-window", self.show_details_window_action ),
			( "refresh-dbs", self.refresh_dbs_action ),
			( "show-stats-window", self.show_stats_window_action ),
			( "show-about", self.show_about_action ),
			( "quit-app", self.quit_app_action )
		]

		self.add_action_entries(action_list)

		# Add keyboard shortcuts
		app.set_accels_for_action("win.toggle-sidebar", ["<ctrl>b"])
		app.set_accels_for_action("win.toggle-infopane", ["<ctrl>i"])
		app.set_accels_for_action("win.search-start", ["<ctrl>f"])
		app.set_accels_for_action("win.search-stop", ["Escape"])
		app.set_accels_for_action("win.search-by-name", ["<ctrl>1"])
		app.set_accels_for_action("win.search-by-desc", ["<ctrl>2"])
		app.set_accels_for_action("win.search-by-group", ["<ctrl>3"])
		app.set_accels_for_action("win.search-by-deps", ["<ctrl>4"])
		app.set_accels_for_action("win.search-by-optdeps", ["<ctrl>5"])
		app.set_accels_for_action("win.search-by-provides", ["<ctrl>6"])
		app.set_accels_for_action("win.view-prev-package", ["<alt>Left"])
		app.set_accels_for_action("win.view-next-package", ["<alt>Right"])
		app.set_accels_for_action("win.show-details-window", ["Return", "KP_Enter"])
		app.set_accels_for_action("win.refresh-dbs", ["F5"])
		app.set_accels_for_action("win.show-stats-window", ["<ctrl>S"])
		app.set_accels_for_action("win.show-about", ["F1"])
		app.set_accels_for_action("win.quit-app", ["<ctrl>q"])

		# Add items to package column view
		self.column_view.model.splice(0, len(self.column_view.model), app.pkg_objects)

		# Initialize sidebar listboxes
		self.init_sidebar()

		# Set status bar search by text
		self.init_search_by_label()

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
			self.repo_listbox.append(SidebarListBoxRow(icon_name="package-x-generic-symbolic", label_text=db if db.isupper() else str.title(db), str_id=db))

		# Select initial repo/status
		self.repo_listbox.select_row(self.repo_listbox_all)
		self.status_listbox.select_row(self.status_listbox_installed)

	def init_search_by_label(self):
		self.search_params_label.set_text(f'Search by: {", ".join(self.column_view.search_params) if self.column_view.search_params != [] else "(none)"}')

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

		self.column_view.search_params = [n for n in ["name", "desc", "group", "deps", "optdeps", "provides"] if self.column_view.get_property(f'search_by_{n}') == True]

		self.init_search_by_label()

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

	def show_stats_window_action(self, action, value, user_data):
		stats_window = StatsWindow()
		stats_window.set_transient_for(self)

		total_count = 0
		total_size = 0

		for db in app.db_names:
			pkg_list = [pkg for pkg in app.pkg_objects if pkg.repository == db and (pkg.status_flags & PkgStatus.INSTALLED)]

			count = len(pkg_list)
			total_count += count

			size = sum([pkg.install_size_raw for pkg in pkg_list])
			total_size += size

			stats_window.model.append(StatsItem(
				db if db.isupper() else str.title(db),
				count,
				f'{size/(1024.0*1024.0):.0f} MiB'
			))

		stats_window.model.append(StatsItem(
			"<b>Total</b>",
			f'<b>{total_count}</b>',
			f'<b>{total_size/(1024.0*1024.0):.0f} MiB</b>'
		))

		stats_window.show()

	def show_about_action(self, action, value, user_data):
		about_window = Adw.AboutWindow(
			application_name="PacView",
			application_icon="software-properties",
			developer_name="draKKar1969",
			version="1.0beta",
			comments="A Pacman database and AUR browser for Arch Linux, heavily inspired by <a href='https://osdn.net/projects/pkgbrowser/'>PkgBrowser</a>",
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

			self.column_view.status_filter.changed(Gtk.FilterChange.DIFFERENT)

	@Gtk.Template.Callback()
	def on_search_started(self, entry):
		self.header_stack.set_visible_child_name("search")

		self.set_focus(self.header_search_entry)

	@Gtk.Template.Callback()
	def on_search_changed(self, entry):
		self.column_view.current_search = entry.get_text().lower()

		self.column_view.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	@Gtk.Template.Callback()
	def on_search_stopped(self, entry):
		entry.set_text("")

		self.header_stack.set_visible_child_name("title")

#------------------------------------------------------------------------------
#-- CLASS: LAUNCHERAPP
#------------------------------------------------------------------------------
class LauncherApp(Adw.Application):
	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, **kwargs):
		super().__init__(**kwargs)

		# Connect signal handlers
		self.connect("activate", self.on_activate)

		self.populate_pkg_objects()

	def populate_pkg_objects(self):
		# Clear PkgOBject list
		self.pkg_objects = []

		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", "/var/lib/pacman")

		# Define sync database names
		self.sync_db_names = ["core", "extra", "community", "multilib"]

		# Get list of configured database names
		dbs = subprocess.run(shlex.split(f'pacman-conf -l'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		self.db_names = [n for n in str(dbs.stdout, 'utf-8').split('\n') if n != ""]

		# Package dict
		self.db_dict = {}

		# Add sync packages
		for db in self.db_names:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				self.db_dict.update(dict([(pkg.name, pkg) for pkg in sync_db.pkgcache]))

		# Add local packages
		local_db = alpm_handle.get_localdb()
		local_dict = dict([(pkg.name, pkg) for pkg in local_db.pkgcache])

		self.db_dict.update(dict([(pkg.name, pkg) for pkg in local_db.pkgcache if pkg.name not in self.db_dict.keys()]))

		# Populate PkgObject list
		def get_local_data(name):
			if name in local_dict.keys():
				local_pkg = local_dict[name]

				status_flags = PkgStatus.NONE

				if local_pkg.reason == 0: status_flags = PkgStatus.EXPLICIT
				else:
					if local_pkg.compute_requiredby() != []:
						status_flags = PkgStatus.DEPENDENCY
					else:
						status_flags = PkgStatus.OPTIONAL if local_pkg.compute_optionalfor() != [] else PkgStatus.ORPHAN

				return(local_pkg, status_flags)

			return(None, PkgStatus.NONE)

		self.pkg_objects = [PkgObject(pkg, get_local_data(pkg.name)) for pkg in self.db_dict.values()]

		# Add AUR to database names
		self.db_names.append("AUR")

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
