#!/usr/bin/env python

import gi, os, datetime, re

from gi.repository import GObject, GLib

import pyalpm

from enum import IntFlag

#------------------------------------------------------------------------------
#-- FLAGS: PKGSTATUS
#------------------------------------------------------------------------------
class PkgStatus(IntFlag):
	EXPLICIT = 1
	DEPENDENCY = 2
	OPTIONAL = 4
	ORPHAN = 8
	INSTALLED = 15
	NONE = 16
	ALL = 31
	UPDATES = 32

#------------------------------------------------------------------------------
#-- CLASS: PKGOBJECT
#------------------------------------------------------------------------------
class PkgObject(GObject.Object):
	__gtype_name__ = "PkgObject"

	#-----------------------------------
	# Internal pyalpm package properties
	#-----------------------------------
	pkg = GObject.Property(type=GObject.TYPE_PYOBJECT, default=None)
	local_pkg = GObject.Property(type=GObject.TYPE_PYOBJECT, default=None)

	#-----------------------------------
	# Status flags property
	#-----------------------------------
	status_flags = GObject.Property(type=int, default=PkgStatus.NONE)

	#-----------------------------------
	# External read-only properties
	#-----------------------------------
	@GObject.Property(type=str, default="")
	def name(self):
		return(self.pkg.name)

	@GObject.Property(type=str, default="")
	def version(self):
		return(self.local_pkg.version if self.local_pkg is not None else self.pkg.version)

	@GObject.Property(type=str, default="")
	def description(self):
		return(self.pkg.desc)

	@GObject.Property(type=str, default="")
	def url(self):
		return(self.url_to_link(self.pkg.url))

	@GObject.Property(type=str, default="")
	def package_url(self):
		if self.pkg.db.name == "local":
			return(self.url_to_link(f'https://aur.archlinux.org/packages/{self.name}'))
		else:
			return(self.url_to_link(f'https://www.archlinux.org/packages/{self.repository}/{self.architecture}/{self.name}'))

	@GObject.Property(type=str, default="")
	def licenses(self):
		return(', '.join(sorted(self.pkg.licenses)))

	@GObject.Property(type=str, default="")
	def status(self):
		if self.status_flags & PkgStatus.EXPLICIT: return("explicit")
		elif self.status_flags & PkgStatus.DEPENDENCY: return("dependency")
		elif self.status_flags & PkgStatus.OPTIONAL: return("optional")
		elif self.status_flags & PkgStatus.ORPHAN: return("orphan")
		else: return("")

	@GObject.Property(type=str, default="")
	def status_icon(self):
		if self.status_flags & PkgStatus.EXPLICIT: return("pkg-explicit")
		elif self.status_flags & PkgStatus.DEPENDENCY: return("pkg-dependency")
		elif self.status_flags & PkgStatus.OPTIONAL: return("pkg-optional")
		elif self.status_flags & PkgStatus.ORPHAN: return("pkg-orphan")
		else: return("")

	@GObject.Property(type=str, default="")
	def repository(self):
		return(self.pkg.db.name if self.pkg.db.name != "local" else "AUR")

	@GObject.Property(type=str, default="")
	def group(self):
		return(', '.join(sorted(self.pkg.groups)))

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def provides_list(self):
		return(self.pkg.provides)

	@GObject.Property(type=str, default="")
	def provides(self):
		return('   '.join(sorted(self.pkg.provides)) if self.pkg.provides != [] else "None")

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def depends_list(self):
		return(self.pkg.depends)

	@GObject.Property(type=str, default="")
	def depends(self):
		return(self.pkglist_to_linkstr(self.pkg.depends))

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def optdepends_list(self):
		return(self.pkg.optdepends)

	@GObject.Property(type=str, default="")
	def optdepends(self):
		return(self.pkglist_to_linkstr(self.pkg.optdepends))

	@GObject.Property(type=str, default="")
	def required_by(self):
		return(self.pkglist_to_linkstr(self.local_pkg.compute_requiredby() if self.local_pkg is not None else self.pkg.compute_requiredby()))

	@GObject.Property(type=str, default="")
	def optional_for(self):
		return(self.pkglist_to_linkstr(self.local_pkg.compute_optionalfor() if self.local_pkg is not None else self.pkg.compute_optionalfor()))

	@GObject.Property(type=str, default="")
	def conflicts(self):
		return(self.pkglist_to_linkstr(self.pkg.conflicts))

	@GObject.Property(type=str, default="")
	def replaces(self):
		return(self.pkglist_to_linkstr(self.pkg.replaces))

	@GObject.Property(type=str, default="")
	def architecture(self):
		return(self.pkg.arch)

	@GObject.Property(type=str, default="")
	def maintainer(self):
		return(self.email_to_link(self.pkg.packager))

	@GObject.Property(type=str, default="")
	def build_date_long(self):
		return(self.date_to_str_long(self.pkg.builddate))

	@GObject.Property(type=int, default=0)
	def install_date_raw(self):
		return(self.local_pkg.installdate if self.local_pkg is not None else self.pkg.installdate)

	@GObject.Property(type=str, default="")
	def install_date_short(self):
		return(self.date_to_str_short(self.install_date_raw))

	@GObject.Property(type=str, default="")
	def install_date_long(self):
		return(self.date_to_str_long(self.install_date_raw))

	@GObject.Property(type=str, default="")
	def download_size(self):
		return(self.size_to_str(self.pkg.size) if self.local_pkg is None else "")

	@GObject.Property(type=int, default=0)
	def install_size_raw(self):
		return(self.pkg.isize)

	@GObject.Property(type=str, default="")
	def install_size(self):
		return(self.size_to_str(self.pkg.isize))

	@GObject.Property(type=str, default="")
	def install_script(self):
		return("Yes" if self.pkg.has_scriptlet else "No")

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def files_list(self):
		return([f[0] for f in self.local_pkg.files] if self.local_pkg is not None else [])

	@GObject.Property(type=str, default="")
	def sha256sum(self):
		return(self.pkg.sha256sum if self.pkg.sha256sum is not None else "")

	@GObject.Property(type=str, default="")
	def md5sum(self):
		return(self.pkg.md5sum if self.pkg.md5sum is not None else "")

	#-----------------------------------
	# Update properties
	#-----------------------------------
	has_updates = GObject.Property(type=bool, default=False)
	update_version = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg, local_data, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.pkg = pkg
		self.local_pkg, self.status_flags = local_data

	#-----------------------------------
	# Helper functions
	#-----------------------------------
	@staticmethod
	def date_to_str_short(value):
		return(datetime.datetime.fromtimestamp(value).strftime("%Y/%m/%d %H:%M") if value != 0 else "")

	@staticmethod
	def date_to_str_long(value):
		return(datetime.datetime.fromtimestamp(value).strftime("%a %d %b %Y %H:%M:%S") if value != 0 else "")

	@staticmethod
	def size_to_str(value, decimals=1):
		if value == 0: return "0 B"
		
		pkg_size = value

		for unit in ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']:
			if pkg_size < 1024.0 or unit == 'PiB':
				break
			pkg_size /= 1024.0
		
		return(f"{pkg_size:.{decimals}f} {unit}")

	@staticmethod
	def url_to_link(url):
		return(f'<a href="{url}">{url}</a>')

	@staticmethod
	def pkglist_to_linkstr(pkglist):
		re_match = "(^|   )([a-zA-Z0-9@._+-]+)(?=&gt;|&lt;|<|>|=|:|   |$)"
		re_res = r"\1<a href='pkg://\2'>\2</a>"
		join_str = GLib.markup_escape_text('   '.join(sorted(pkglist)))

		return(re.sub(re_match, re_res, join_str) if pkglist != [] else "None")

	@staticmethod
	def email_to_link(email):
		return(re.sub("([^<]+)<?([^>]+)?>?", r"\1<a href='mailto:\2'>\2</a>", email))

#------------------------------------------------------------------------------
#-- CLASS: PKGPROPERTY
#------------------------------------------------------------------------------
class PkgProperty(GObject.Object):
	__gtype_name__ = "PkgProperty"

	#-----------------------------------
	# Read/write properties
	#-----------------------------------
	prop_name = GObject.Property(type=str, default="")
	prop_value = GObject.Property(type=str, default="")
	prop_icon = GObject.Property(type=str, default="")
	prop_copy = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, name, value, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.prop_name = name
		self.prop_value = value

#------------------------------------------------------------------------------
#-- CLASS: STATSITEM
#------------------------------------------------------------------------------
class StatsItem(GObject.Object):
	__gtype_name__ = "StatsItem"

	#-----------------------------------
	# Read/write properties
	#-----------------------------------
	repository = GObject.Property(type=str, default="")
	count = GObject.Property(type=str, default="")
	size = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, repository, count, size, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.repository = repository
		self.count = count
		self.size = size
