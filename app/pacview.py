#!/usr/bin/env python

import gi, sys, os, urllib.parse, subprocess, shlex, re, threading, textwrap, hashlib

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject, Pango, Gdk, GLib

import pyalpm

from objects import PkgStatus, PkgObject, PkgProperty, PkgBackup, StatsItem

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

		# Initialize widgets
		total_count = 0
		total_size = 0

		for db in app.main_window.pacman_db_names:
			obj_list = [obj for obj in app.main_window.pkg_objects if obj.repository == db and (obj.status_flags & PkgStatus.INSTALLED)]

			count = len(obj_list)
			total_count += count

			size = sum([obj.install_size_raw for obj in obj_list])
			total_size += size

			self.model.append(StatsItem(
				db if db.isupper() else str.title(db),
				count,
				PkgObject.size_to_str(size, 2)
			))

		self.model.append(StatsItem(
			"<b>Total</b>",
			f'<b>{total_count}</b>',
			f'<b>{PkgObject.size_to_str(total_size, 2)}</b>'
		))

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
#-- CLASS: STACKTOGGLEBUTTON
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/stacktogglebutton.ui")
class StackToggleButton(Gtk.ToggleButton):
	__gtype_name__ = "StackToggleButton"

	#-----------------------------------
	# Properties
	#-----------------------------------
	icon = GObject.Property(type=str, default="")
	text = GObject.Property(type=str, default="")
	orientation = GObject.Property(type=Gtk.Orientation, default=Gtk.Orientation.HORIZONTAL)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: PKGDETAILSWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgdetailswindow.ui")
class PkgDetailsWindow(Adw.ApplicationWindow):
	__gtype_name__ = "PkgDetailsWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	pkg_label = Gtk.Template.Child()

	content_stack = Gtk.Template.Child()

	file_header_label = Gtk.Template.Child()
	files_model = Gtk.Template.Child()

	tree_label = Gtk.Template.Child()
	tree_dropdown = Gtk.Template.Child()

	log_model = Gtk.Template.Child()

	cache_header_label = Gtk.Template.Child()
	cache_model = Gtk.Template.Child()

	backup_header_label = Gtk.Template.Child()
	backup_view = Gtk.Template.Child()
	backup_model = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg_object, monospace_font, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.pkg_object = pkg_object

		# Create stack action
		action = Gio.SimpleAction.new_stateful("switch-stack-page", GLib.VariantType.new("s"), GLib.Variant.new_string("files"))
		action.connect("change-state", self.on_stack_page_switch)
		self.add_action(action)

		# Set tree label font
		if monospace_font == "":
			gsettings = Gio.Settings(schema_id="org.gnome.desktop.interface")

			monospace_font = gsettings.get_string("monospace-font-name")

		self.tree_label.set_attributes(Pango.AttrList.from_string(f'0 -1 font-desc "{monospace_font}"'))

		# Initialize widgets
		if pkg_object is not None:
			# Set package name
			self.pkg_label.set_text(f'{pkg_object.repository}/{pkg_object.name}')

			# Populate file list
			self.file_header_label.set_text(f'Files ({len(pkg_object.files)})')

			file_list = [f'/{f}' for f in pkg_object.files]

			self.files_model.splice(0, 0, file_list)

			# Populate dependency tree
			self.populate_dep_tree()

			# Populate log
			pkg_log = subprocess.run(shlex.split(f'/usr/bin/paclog --no-color --package={pkg_object.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			log_lines = [re.sub("\[(.+)T(.+)\+.+\] (.+)", r"\1 \2 : \3", l) for l in pkg_log.stdout.decode().split('\n') if l != ""]

			self.log_model.splice(0, 0, log_lines[::-1]) # Reverse list

			# Populate cache
			pkg_cache = subprocess.run(shlex.split(f'/usr/bin/paccache -vdk0 {pkg_object.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			cache_lines = [l for l in pkg_cache.stdout.decode().split('\n') if (l != "" and not l.startswith("==>") and not l.endswith(".sig"))]

			self.cache_header_label.set_text(f'Cache ({len(cache_lines)})')
			self.cache_model.splice(0, 0, cache_lines)

			# Hide backup column view header
			child = self.backup_view.get_first_child()
			
			if child is not None and type(child).__name__ == "GtkListItemWidget":
				child.set_visible(False)

			# Populate backup list
			self.backup_header_label.set_text(f'Backup Files ({len(pkg_object.backup)})')

			backup_list = []

			for bk in pkg_object.backup:
				filename = f'/{bk[0]}'
				status = ""

				md5_hash = hashlib.md5()

				try:
					with open(filename, "rb") as f:
						# Read and update hash in chunks of 4K
						for block in iter(lambda: f.read(4096), b""):
							md5_hash.update(block)
							
						text_hash = md5_hash.hexdigest()

						status_icon = "backup-unchanged" if text_hash == bk[1] else "backup-changed"
						status = "unchanged" if text_hash == bk[1] else "changed"
				except:
					status_icon = "backup-error"
					status = "read error"

				backup_list += [PkgBackup(filename, status_icon, status)]

			if backup_list != []:
				self.backup_model.splice(0, 0, backup_list)

			self.backup_view.set_visible(backup_list != [])

	#-----------------------------------
	# Populate dependency tree function
	#-----------------------------------
	def populate_dep_tree(self):
		depth = self.tree_dropdown.get_selected_item().get_string()
		depth_flag = "" if depth == "Default" else f'-d {depth}'

		pkg_tree = subprocess.run(shlex.split(f'/usr/bin/pactree {"" if (self.pkg_object.status_flags & PkgStatus.INSTALLED) else " -s"} {depth_flag} {self.pkg_object.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		self.tree_label.set_label(re.sub(" provides.+", "", pkg_tree.stdout.decode()))

	#-----------------------------------
	# Action handlers
	#-----------------------------------
	def on_stack_page_switch(self, action, value):
		self.content_stack.set_visible_child_name(value.get_string())

	#-----------------------------------
	# Dependency tree dropdown signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_tree_depth_changed(self, dropdown, prop):
		if self.pkg_object is not None:
			self.populate_dep_tree()

	#-----------------------------------
	# Key press signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_key_pressed(self, keyval, keycode, user_data, state):
		if keycode == Gdk.KEY_Escape and state == 0: self.close()

#------------------------------------------------------------------------------
#-- CLASS: PKGINFOPANE
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkginfopane.ui")
class PkgInfoPane(Gtk.Overlay):
	__gtype_name__ = "PkgInfoPane"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	model = Gtk.Template.Child()

	nav_button_box = Gtk.Template.Child()
	prev_button = Gtk.Template.Child()
	next_button = Gtk.Template.Child()
	details_button = Gtk.Template.Child()

	empty_label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	__obj_list = []
	__obj_index = -1

	@GObject.Property(type=PkgObject, default=None)
	def pkg_object(self):
		return(self.__obj_list[self.__obj_index] if (self.__obj_index >= 0 and self.__obj_index < len(self.__obj_list)) else None)

	@pkg_object.setter
	def pkg_object(self, value):
		self.__obj_list = [value]
		self.__obj_index = 0

		self.display_package(value)

		self.nav_button_box.set_visible(False)

		self.empty_label.set_visible(value is None)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Hide column view header
		child = self.view.get_first_child()

		if child is not None and type(child).__name__ == "GtkListItemWidget":
			child.set_visible(False)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_setup_value(self, factory, item):
		image = Gtk.Image()

		button = Gtk.Button(icon_name="edit-copy")
		button.add_css_class("flat")
		button.add_css_class("inline-button")
		button.set_can_focus(False)
		button.connect("clicked", self.on_copybtn_clicked)

		label = Gtk.Label(hexpand=True, xalign=0, use_markup=True)
		label.connect("activate-link", self.on_link_activated)

		box = Gtk.Box(spacing=6)
		box.append(image)
		box.append(button)
		box.append(label)

		item.set_child(box)

	@Gtk.Template.Callback()
	def on_bind_value(self, factory, item):
		child = item.get_child()
		obj = item.get_item()
		
		image = child.get_first_child()
		button = child.get_first_child().get_next_sibling()
		label = child.get_last_child()

		icon = obj.icon

		image.set_visible(icon != "")
		image.set_from_icon_name(icon)

		label.set_label(obj.value)

		button.set_visible(obj.can_copy)

	#-----------------------------------
	# Link signal handler
	#-----------------------------------
	def on_link_activated(self, label, url):
		parse_url = urllib.parse.urlsplit(url)

		if parse_url.scheme != "pkg": return(False)

		pkg_name = parse_url.netloc

		obj_dict = {obj.name: obj for obj in app.main_window.pkg_objects}

		new_obj = None

		if pkg_name in obj_dict.keys():
			new_obj = obj_dict[pkg_name]
		else:
			for obj in obj_dict.values():
				if any(pkg_name in s for s in obj.provides):
					new_obj = obj
					break

		if new_obj is not None and new_obj is not self.__obj_list[self.__obj_index]:
			self.__obj_list = self.__obj_list[:self.__obj_index+1]
			self.__obj_list.append(new_obj)

			self.__obj_index += 1

			self.display_package(new_obj)

			self.nav_button_box.set_visible(True)

		return(True)

	#-----------------------------------
	# Copy button signal handler
	#-----------------------------------
	def on_copybtn_clicked(self, button):
		clipboard = self.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, button.get_next_sibling().get_label()))

		clipboard.set_content(content)

	#-----------------------------------
	# Display functions
	#-----------------------------------
	def display_package(self, obj):
		self.prev_button.set_sensitive(self.__obj_index > 0)
		self.next_button.set_sensitive(self.__obj_index < len(self.__obj_list) - 1)

		self.model.remove_all()

		if obj is not None:
			self.model.append(PkgProperty("Name", f'<b>{obj.name}</b>'))
			if obj.update_version != "": self.model.append(PkgProperty("Version", obj.update_version, icon="pkg-update"))
			else: self.model.append(PkgProperty("Version", obj.version))
			self.model.append(PkgProperty("Description", GLib.markup_escape_text(obj.description)))
			self.model.append(PkgProperty("URL", self.url_to_link(obj.url)))
			if obj.repository in app.main_window.sync_db_names: self.model.append(PkgProperty("Package URL", self.url_to_link(f'https://www.archlinux.org/packages/{obj.repository}/{obj.architecture}/{obj.name}')))
			elif obj.repository == "AUR": self.model.append(PkgProperty("AUR URL", self.url_to_link(f'https://aur.archlinux.org/packages/{obj.name}')))
			self.model.append(PkgProperty("Licenses", GLib.markup_escape_text(obj.licenses)))
			self.model.append(PkgProperty("Status", obj.status if (obj.status_flags & PkgStatus.INSTALLED) else "not installed", icon=obj.status_icon))
			self.model.append(PkgProperty("Repository", obj.repository))
			if obj.group != "":self.model.append(PkgProperty("Groups", obj.group))
			if obj.provides != []: self.model.append(PkgProperty("Provides", self.wrap_escape_list(obj.provides)))
			self.model.append(PkgProperty("Dependencies ", self.pkglist_to_linkstr(obj.depends)))
			if obj.optdepends != []: self.model.append(PkgProperty("Optional", self.pkglist_to_linkstr(obj.optdepends)))
			self.model.append(PkgProperty("Required By", self.pkglist_to_linkstr(obj.required_by)))
			if obj.optional_for != []: self.model.append(PkgProperty("Optional For", self.pkglist_to_linkstr(obj.optional_for)))
			if obj.conflicts != []: self.model.append(PkgProperty("Conflicts With", self.pkglist_to_linkstr(obj.conflicts)))
			if obj.replaces != []: self.model.append(PkgProperty("Replaces", self.pkglist_to_linkstr(obj.replaces)))
			self.model.append(PkgProperty("Architecture", obj.architecture))
			self.model.append(PkgProperty("Maintainer", self.email_to_link(obj.maintainer)))
			self.model.append(PkgProperty("Build Date", obj.build_date_long))
			if obj.install_date_long != "": self.model.append(PkgProperty("Install Date", obj.install_date_long))
			if obj.download_size != "": self.model.append(PkgProperty("Download Size", obj.download_size))
			self.model.append(PkgProperty("Installed Size", obj.install_size))
			self.model.append(PkgProperty("Install Script", "Yes" if obj.install_script else "No"))
			if obj.sha256sum is not None: self.model.append(PkgProperty("SHA256 Sum", obj.sha256sum, can_copy=True))
			if obj.md5sum is not None: self.model.append(PkgProperty("MD5 Sum", obj.md5sum, can_copy=True))

	def display_prev_package(self):
		if self.__obj_index > 0:
			self.__obj_index -=1

			self.display_package(self.pkg_object)

	def display_next_package(self):
		if self.__obj_index < len(self.__obj_list) - 1:
			self.__obj_index +=1

			self.display_package(self.pkg_object)

	#-----------------------------------
	# Helper functions
	#-----------------------------------
	@staticmethod
	def url_to_link(url):
		escaped_url = GLib.markup_escape_text(url)
		return(f'<a href="{escaped_url}">{escaped_url}</a>')

	@staticmethod
	def email_to_link(email):
		return(re.sub("([^<]+)<?([^>]+)?>?", r"\1<a href='mailto:\2'>\2</a>", email))

	@staticmethod
	def wrap_escape_list(pkglist, wrap_width=150):
		return(GLib.markup_escape_text(textwrap.fill('   '.join(sorted(pkglist)), width=wrap_width, break_on_hyphens=False)))

	@staticmethod
	def pkglist_to_linkstr(pkglist):
		if pkglist == []: return("None")

		re_match = "(^|   )([a-zA-Z0-9@._+-]+)(?=&gt;|&lt;|<|>|=|:|   |$)"
		re_res = r"\1<a href='pkg://\2'>\2</a>"
		join_str = PkgInfoPane.wrap_escape_list(pkglist)

		return(re.sub(re_match, re_res, join_str, flags=re.MULTILINE))

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgcolumnview.ui")
class PkgColumnView(Gtk.Overlay):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	selection = Gtk.Template.Child()
	filter_model = Gtk.Template.Child()
	model = Gtk.Template.Child()
	empty_label = Gtk.Template.Child()

	repo_filter = Gtk.Template.Child()
	status_filter = Gtk.Template.Child()
	search_filter = Gtk.Template.Child()

	package_column = Gtk.Template.Child()
	version_column = Gtk.Template.Child()
	repository_column = Gtk.Template.Child()
	status_column = Gtk.Template.Child()
	date_column = Gtk.Template.Child()
	size_column = Gtk.Template.Child()
	group_column = Gtk.Template.Child()

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
	search_by_files = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind item count to empty label visibility
		self.selection.bind_property(
			"n-items", self.empty_label, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value == 0
		)

		# Set filter functions
		self.status_filter.set_filter_func(self.filter_by_status)
		self.search_filter.set_filter_func(self.filter_by_search)

		# Connect property change signal handlers
		self.connect("notify::current-status", self.on_current_status_changed)

		self.connect("notify::current-search", self.on_current_search_changed)

		self.connect("notify::search-by-name", self.on_search_by_changed)
		self.connect("notify::search-by-desc", self.on_search_by_changed)
		self.connect("notify::search-by-group", self.on_search_by_changed)
		self.connect("notify::search-by-deps", self.on_search_by_changed)
		self.connect("notify::search-by-optdeps", self.on_search_by_changed)
		self.connect("notify::search-by-provides", self.on_search_by_changed)
		self.connect("notify::search-by-files", self.on_search_by_changed)

		# Sort view by name (first) column
		self.view.sort_by_column(self.view.get_columns()[0], Gtk.SortType.ASCENDING)

	#-----------------------------------
	# Filter functions
	#-----------------------------------
	def filter_by_status(self, item):
		return(item.status_flags & self.current_status)

	def filter_by_search(self, item):
		if self.current_search == "":
			return(True)
		else:
			search_term = self.current_search.lower()

			return(any((
				self.search_by_name and search_term in item.name.lower(),
				self.search_by_desc and search_term in item.description.lower(),
				self.search_by_group and search_term in item.group.lower(),
				self.search_by_deps and any(search_term in s.lower() for s in item.depends),
				self.search_by_optdeps and any(search_term in s.lower() for s in item.optdepends),
				self.search_by_provides and any(search_term in s.lower() for s in item.provides),
				self.search_by_files and any(search_term in s.lower() for s in item.files)
			)))

	#-----------------------------------
	# Property change signal handlers
	#-----------------------------------
	def on_current_status_changed(self, view, prop):
		self.status_filter.changed(Gtk.FilterChange.DIFFERENT)

	def on_current_search_changed(self, view, prop):
		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	def on_search_by_changed(self, view, prop):
		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

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

	#-----------------------------------
	# Properties
	#-----------------------------------
	str_id = GObject.Property(type=str, default="")

	icon = GObject.Property(type=str, default="")
	text = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: SEARCHTAG
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/searchtag.ui")
class SearchTag(Gtk.Label):
	__gtype_name__ = "SearchTag"

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: SEARCHHEADER
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/searchheader.ui")
class SearchHeader(Gtk.Stack):
	__gtype_name__ = "SearchHeader"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	search_entry = Gtk.Template.Child()

	searchtag_box = Gtk.Template.Child()

	filter_image = Gtk.Template.Child()
	filter_popover = Gtk.Template.Child()

	searchtag_name = Gtk.Template.Child()
	searchtag_desc = Gtk.Template.Child()
	searchtag_group = Gtk.Template.Child()
	searchtag_deps = Gtk.Template.Child()
	searchtag_optdeps = Gtk.Template.Child()
	searchtag_provides = Gtk.Template.Child()
	searchtag_files = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	title = GObject.Property(type=str, default="")

	@GObject.Property(type=Gtk.Widget, default=None)
	def key_capture_widget(self):
		return(self.search_entry.get_key_capture_widget())

	@key_capture_widget.setter
	def key_capture_widget(self, value):
		self.search_entry.set_key_capture_widget(value)

	search_active = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind property change signal handlers
		self.connect("notify::search-active", self.on_search_active_changed)

		# Position search tags
		Gtk.Widget.insert_after(self.searchtag_box, self.search_entry, self.search_entry.get_first_child())

		# Capture click on filter image
		click_gesture = Gtk.GestureClick.new()
		click_gesture.connect("pressed", self.on_filter_image_clicked)

		self.filter_image.add_controller(click_gesture)

	def on_filter_image_clicked(self, n_press, x, y, user_data):
		self.filter_popover.popup()
		self.filter_popover.grab_focus()

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_search_started(self, entry):
		self.search_active = True

	@Gtk.Template.Callback()
	def on_search_stopped(self, entry):
		self.search_active = False

	#-----------------------------------
	# Property change signal handlers
	#-----------------------------------
	def on_search_active_changed(self, view, prop):
		if self.search_active == True:
			self.set_visible_child_name("search")

			self.search_entry.grab_focus()
		else:
			self.search_entry.set_text("")

			self.set_visible_child_name("title")

#------------------------------------------------------------------------------
#-- CLASS: MAINWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/mainwindow.ui")
class MainWindow(Adw.ApplicationWindow):
	__gtype_name__ = "MainWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	header_search = Gtk.Template.Child()

	header_sidebar_btn = Gtk.Template.Child()
	header_infopane_btn = Gtk.Template.Child()
	header_search_btn = Gtk.Template.Child()

	flap = Gtk.Template.Child()

	repo_listbox = Gtk.Template.Child()
	status_listbox = Gtk.Template.Child()

	pane = Gtk.Template.Child()

	column_view = Gtk.Template.Child()
	info_pane = Gtk.Template.Child()

	status_count_label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	monospace_font = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		#-----------------------------
		# GSettings
		#-----------------------------
		# Bind gsettings
		self.gsettings = Gio.Settings(schema_id="com.github.PacView")

		self.gsettings.bind("window-width", self, "default-width", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("window-height", self, "default-height", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("window-maximized", self, "maximized",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-sidebar", self.flap, "reveal_flap",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-infopane", self.info_pane, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("infopane-position", self.pane, "position",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-column-version", self.column_view.version_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-column-repository", self.column_view.repository_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-column-status", self.column_view.status_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-column-date", self.column_view.date_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-column-size", self.column_view.size_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-column-group", self.column_view.group_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("monospace-font", self, "monospace_font",Gio.SettingsBindFlags.DEFAULT)

		#-----------------------------
		# Toolbar buttons
		#-----------------------------
		# Bind toolbar search button state to header search active state
		self.header_search_btn.bind_property(
			"active", self.header_search, "search_active",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		# Create toolbar button actions
		self.add_action(self.gsettings.create_action("show-sidebar"))
		self.add_action(self.gsettings.create_action("show-infopane"))

		app.set_accels_for_action("win.show-sidebar", ["<ctrl>b"])
		app.set_accels_for_action("win.show-infopane", ["<ctrl>i"])

		action_list = [
			( "search-start", self.start_search_action ),
			( "search-stop", self.stop_search_action )
		]

		self.add_action_entries(action_list)

		app.set_accels_for_action("win.search-start", ["<ctrl>f"])
		app.set_accels_for_action("win.search-stop", ["Escape"])

		#-----------------------------
		# Search header
		#-----------------------------
		# Bind header search term to column view
		self.header_search.search_entry.bind_property(
			"text", self.column_view, "current_search",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		#-----------------------------
		# Column view
		#-----------------------------
		# Bind column view selected item to info pane
		self.column_view.selection.bind_property(
			"selected-item", self.info_pane, "pkg_object",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		# Bind column view count to status label text
		self.column_view.filter_model.bind_property(
			"n-items", self.status_count_label, "label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: f'{value} matching package{"s" if value != 1 else ""}'
		)

		# Bind column view search by properties to status search labels visibility
		self.column_view.bind_property(
			"search_by_name", self.header_search.searchtag_name, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_desc", self.header_search.searchtag_desc, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_group", self.header_search.searchtag_group, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_deps", self.header_search.searchtag_deps, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_optdeps", self.header_search.searchtag_optdeps, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_provides", self.header_search.searchtag_provides, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_files", self.header_search.searchtag_files, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		# Create column view header menu actions
		self.add_action(self.gsettings.create_action("show-column-version"))
		self.add_action(self.gsettings.create_action("show-column-repository"))
		self.add_action(self.gsettings.create_action("show-column-status"))
		self.add_action(self.gsettings.create_action("show-column-date"))
		self.add_action(self.gsettings.create_action("show-column-size"))
		self.add_action(self.gsettings.create_action("show-column-group"))

		# Create column view search filter actions
		self.add_action(Gio.PropertyAction.new("search-by-name", self.column_view, "search_by_name"))
		self.add_action(Gio.PropertyAction.new("search-by-desc", self.column_view, "search_by_desc"))
		self.add_action(Gio.PropertyAction.new("search-by-group", self.column_view, "search_by_group"))
		self.add_action(Gio.PropertyAction.new("search-by-deps", self.column_view, "search_by_deps"))
		self.add_action(Gio.PropertyAction.new("search-by-optdeps", self.column_view, "search_by_optdeps"))
		self.add_action(Gio.PropertyAction.new("search-by-provides", self.column_view, "search_by_provides"))
		self.add_action(Gio.PropertyAction.new("search-by-files", self.column_view, "search_by_files"))

		app.set_accels_for_action("win.search-by-name", ["<ctrl>1"])
		app.set_accels_for_action("win.search-by-desc", ["<ctrl>2"])
		app.set_accels_for_action("win.search-by-group", ["<ctrl>3"])
		app.set_accels_for_action("win.search-by-deps", ["<ctrl>4"])
		app.set_accels_for_action("win.search-by-optdeps", ["<ctrl>5"])
		app.set_accels_for_action("win.search-by-provides", ["<ctrl>6"])
		app.set_accels_for_action("win.search-by-files", ["<ctrl>7"])

		action_list = [
			( "selectall-searchby-params", self.selectall_searchby_params_action ),
			( "reset-searchby-params", self.reset_searchby_params_action )
		]

		self.add_action_entries(action_list)

		app.set_accels_for_action("win.selectall-searchby-params", ["<ctrl>L"])
		app.set_accels_for_action("win.reset-searchby-params", ["<ctrl>R"])

		# Connect column view activate signal
		self.column_view.view.connect("activate", self.on_column_view_activated)

		#-----------------------------
		# Info pane
		#-----------------------------
		# Bind info pane package to details button enabled state
		self.info_pane.bind_property(
			"pkg_object", self.info_pane.details_button, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value is not None
		)

		# Add info pane actions
		action_list = [
			( "view-prev-package", self.view_prev_package_action ),
			( "view-next-package", self.view_next_package_action ),
			( "show-details-window", self.show_details_window_action )
		]

		self.add_action_entries(action_list)

		app.set_accels_for_action("win.view-prev-package", ["<alt>Left"])
		app.set_accels_for_action("win.view-next-package", ["<alt>Right"])
		app.set_accels_for_action("win.show-details-window", ["<alt>Return", "<alt>KP_Enter"])

		#-----------------------------
		# Window
		#-----------------------------
		# Add window actions
		action_list = [
			( "refresh-dbs", self.refresh_dbs_action ),
			( "show-stats-window", self.show_stats_window_action ),
			( "copy-package-list", self.copy_package_list_action ),

			( "show-about", self.show_about_action ),
			( "quit-app", self.quit_app_action )
		]

		self.add_action_entries(action_list)

		app.set_accels_for_action("win.refresh-dbs", ["F5"])
		app.set_accels_for_action("win.show-stats-window", ["<alt>S"])
		app.set_accels_for_action("win.copy-package-list", ["<alt>L"])
		
		app.set_accels_for_action("win.show-about", ["F1"])
		app.set_accels_for_action("win.quit-app", ["<ctrl>q"])

		# Set initial focus on package column view
		self.set_focus(self.column_view.view)

	#-----------------------------------
	# Show window signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_show(self, window):
		self.init_window()

	#-----------------------------------
	# Init window function
	#-----------------------------------
	def init_window(self):
		self.init_databases()

		self.populate_column_view()

		self.populate_sidebar()

		thread = threading.Thread(target=self.checkupdates_async, daemon=True)
		thread.start()

	#-----------------------------------
	# Init databases function
	#-----------------------------------
	def init_databases(self):
		# Define sync database names
		self.sync_db_names = ["core", "extra", "community", "multilib"]

		# Get list of configured database names
		dbs = subprocess.run(shlex.split(f'/usr/bin/pacman-conf -l'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		self.pacman_db_names = [n for n in dbs.stdout.decode().split('\n') if n != ""]

		# Add AUR to configured database names
		self.pacman_db_names.append("AUR")

	#-----------------------------------
	# Populate column view function
	#-----------------------------------
	def populate_column_view(self):
		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", "/var/lib/pacman")

		# Package dict
		all_pkg_dict = {}

		# Add sync packages
		for db in self.pacman_db_names:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				all_pkg_dict.update({pkg.name: pkg for pkg in sync_db.pkgcache})

		# Add local packages
		local_db = alpm_handle.get_localdb()
		local_pkg_dict = {pkg.name: pkg for pkg in local_db.pkgcache}

		all_pkg_dict.update({pkg.name: pkg for pkg in local_db.pkgcache if pkg.name not in all_pkg_dict.keys()})

		# Create list of package objects
		def __get_local_data(name):
			if name in local_pkg_dict.keys():
				local_pkg = local_pkg_dict[name]

				status_flags = PkgStatus.NONE

				if local_pkg.reason == 0: status_flags = PkgStatus.EXPLICIT
				else:
					if local_pkg.compute_requiredby() != []:
						status_flags = PkgStatus.DEPENDENCY
					else:
						status_flags = PkgStatus.OPTIONAL if local_pkg.compute_optionalfor() != [] else PkgStatus.ORPHAN

				return(local_pkg, status_flags)

			return(None, PkgStatus.NONE)

		self.pkg_objects = [PkgObject(pkg, __get_local_data(pkg.name)) for pkg in all_pkg_dict.values()]

		self.column_view.model.splice(0, len(self.column_view.model), self.pkg_objects)

	#-----------------------------------
	# Init sidebar function
	#-----------------------------------
	def populate_sidebar(self):
		# Remove rows from listboxes
		while(row := self.repo_listbox.get_row_at_index(0)):
			self.repo_listbox.remove(row)

		while(row := self.status_listbox.get_row_at_index(0)):
			self.status_listbox.remove(row)

		# Add rows to repository list box
		repo_row = SidebarListBoxRow(icon="repository-symbolic", text="All")
		self.repo_listbox.append(repo_row)

		for db in self.pacman_db_names:
			self.repo_listbox.append(SidebarListBoxRow(icon="repository-symbolic", text=db if db.isupper() else str.title(db), str_id=db))

		self.repo_listbox.select_row(repo_row)

		# Add rows to status list box
		status_dict = {PkgStatus.ALL: "status-all-symbolic", PkgStatus.INSTALLED: "status-installed-symbolic", PkgStatus.EXPLICIT: "status-explicit-symbolic", PkgStatus.DEPENDENCY: "status-dependency-symbolic", PkgStatus.OPTIONAL: "status-optional-symbolic", PkgStatus.ORPHAN: "status-orphan-symbolic", PkgStatus.NONE: "status-none-symbolic", PkgStatus.UPDATES: "status-update-symbolic"}

		for st in status_dict.keys():
			row = SidebarListBoxRow(icon=status_dict.get(st, ""), text=st.name.title(), str_id=st.value)
			self.status_listbox.append(row)
			if st == PkgStatus.INSTALLED:
				self.status_listbox.select_row(row)
			if st == PkgStatus.UPDATES:
				self.update_row = row
				self.update_row.image.set_opacity(0.3)
				self.update_row.set_sensitive(False)

	#-----------------------------------
	# Check for updates functions
	#-----------------------------------
	def checkupdates_async(self):
		# Get updates
		upd = subprocess.run(shlex.split(f'/usr/bin/checkupdates'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		expr = re.compile("(\S+)\s(\S+\s->\s\S+)")

		updates = {expr.sub(r"\1", u): expr.sub(r"\2", u) for u in upd.stdout.decode().split('\n') if u != ""}

		GLib.idle_add(self.show_pkg_updates, updates, upd.returncode)

	def show_pkg_updates(self, updates, returncode):
		if returncode == 0:
			# Modify package object properties if update available
			if len(updates) != 0:
				for obj in self.pkg_objects:
					if obj.name in updates.keys():
						obj.has_updates = True
						obj.status_flags |= PkgStatus.UPDATES
						obj.update_version = updates[obj.name]

			# Force update of info pane package object
			self.info_pane.pkg_object = self.column_view.selection.get_selected_item()

			# Update status
			self.update_row.text = f'Updates ({len(updates)})'
			self.update_row.set_tooltip_text("")
			self.update_row.image.set_from_icon_name("status-update-symbolic")
			self.update_row.image.set_opacity(1.0)
			self.update_row.set_sensitive(True)
		elif returncode == 1:
			self.update_row.text = f'Updates'
			self.update_row.set_tooltip_text("Update error")
			self.update_row.image.set_from_icon_name("status-update-error-symbolic")
			self.update_row.image.set_opacity(0.3)
			self.update_row.set_sensitive(False)
		else:
			self.update_row.text = f'Updates'
			self.update_row.set_tooltip_text("No updates available")
			self.update_row.image.set_from_icon_name("status-update-symbolic")
			self.update_row.image.set_opacity(0.3)
			self.update_row.set_sensitive(False)

		return(False)

	#-----------------------------------
	# Action handlers
	#-----------------------------------
	def start_search_action(self, action, value, user_data):
		self.header_search.search_active = True

	def stop_search_action(self, action, value, user_data):
		self.header_search.search_active = False

		self.column_view.view.grab_focus()

	def selectall_searchby_params_action(self, action, value, user_data):
		for n in ["name", "desc", "group", "deps", "optdeps", "provides", "files"]:
			self.column_view.set_property(f'search_by_{n}', True)

	def reset_searchby_params_action(self, action, value, user_data):
		for n in ["name", "desc", "group", "deps", "optdeps", "provides", "files"]:
			self.column_view.set_property(f'search_by_{n}', (n == "name"))
			
	def view_prev_package_action(self, action, value, user_data):
		self.info_pane.display_prev_package()

	def view_next_package_action(self, action, value, user_data):
		self.info_pane.display_next_package()

	def show_details_window_action(self, action, value, user_data):
		if self.info_pane.pkg_object is not None:
			details_window = PkgDetailsWindow(self.info_pane.pkg_object, self.monospace_font, transient_for=self)
			details_window.show()

	def refresh_dbs_action(self, action, value, user_data):
		self.header_search.search_active = False

		GLib.idle_add(self.init_window)

	def show_stats_window_action(self, action, value, user_data):
		stats_window = StatsWindow(transient_for=self)
		stats_window.show()

	def copy_package_list_action(self, action, value, user_data):
		copy_text = '\n'.join([f'{obj.repository}/{obj.name}' for obj in self.column_view.selection])

		clipboard = self.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, copy_text))

		clipboard.set_content(content)

	def show_about_action(self, action, value, user_data):
		about_window = Adw.AboutWindow(
			application_name="PacView",
			application_icon="software-properties",
			developer_name="draKKar1969",
			version="1.0.rc6",
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

	def on_column_view_activated(self, view, pos):
		self.info_pane.pkg_object = self.column_view.selection.get_selected_item()

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
