#!/usr/bin/env python

import gi, os, datetime

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
	# Status enum property
	#-----------------------------------
	status_enum = GObject.Property(type=int, default=PkgStatus.NONE)

	#-----------------------------------
	# External read-only properties
	#-----------------------------------
	@GObject.Property(type=str, default="")
	def name(self):
		return(self.pkg.name)

	@GObject.Property(type=str, default="")
	def version(self):
		return(self.pkg.version)

	@GObject.Property(type=str, default="")
	def description(self):
		return(self.pkg.desc)

	@GObject.Property(type=str, default="")
	def url(self):
		return(self.url_to_link(self.pkg.url))

	@GObject.Property(type=str, default="")
	def package_url(self):
		return(self.url_to_link(f'https://www.archlinux.org/packages/{self.repository}/{self.architecture}/{self.name}'))

	@GObject.Property(type=str, default="")
	def licenses(self):
		return(', '.join(sorted(self.pkg.licenses)))

	@GObject.Property(type=str, default="")
	def status(self):
		str_dict = { PkgStatus.EXPLICIT: "explicit", PkgStatus.DEPENDENCY: "dependency", PkgStatus.OPTIONAL: "optional", PkgStatus.ORPHAN: "orphan" }

		return(str_dict.get(self.status_enum, ""))

	@GObject.Property(type=str, default="")
	def status_icon(self):
		icon_dict = { PkgStatus.EXPLICIT: "package-install", PkgStatus.DEPENDENCY: "package-installed-updated", PkgStatus.OPTIONAL: "package-installed-outdated", PkgStatus.ORPHAN: "package-purge" }

		return(icon_dict.get(self.status_enum, ""))

	@GObject.Property(type=str, default="")
	def repository(self):
		return(self.pkg.db.name)

	@GObject.Property(type=str, default="")
	def group(self):
		return(', '.join(sorted(self.pkg.groups)))

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def provides_list(self):
		return(self.pkg.provides)

	@GObject.Property(type=str, default="")
	def provides(self):
		return(self.pkglist_to_linkstr(self.pkg.provides))

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
		return(GLib.markup_escape_text(self.pkg.packager))

	@GObject.Property(type=str, default="")
	def build_date_long(self):
		return(self.int_to_datestr_long(self.pkg.builddate))

	@GObject.Property(type=int, default=0)
	def install_date_raw(self):
		return(self.local_pkg.installdate if self.local_pkg is not None else self.pkg.installdate)

	@GObject.Property(type=str, default="")
	def install_date_short(self):
		return(self.int_to_datestr_short(self.install_date_raw))

	@GObject.Property(type=str, default="")
	def install_date_long(self):
		return(self.int_to_datestr_long(self.install_date_raw))

	@GObject.Property(type=str, default="")
	def download_size(self):
		return(self.int_to_sizestr(self.pkg.size) if self.local_pkg is None else "")

	@GObject.Property(type=int, default=0)
	def install_size_raw(self):
		return(self.pkg.isize)

	@GObject.Property(type=str, default="")
	def install_size(self):
		return(self.int_to_sizestr(self.pkg.isize))

	@GObject.Property(type=str, default="")
	def install_script(self):
		return("Yes" if self.pkg.has_scriptlet else "No")

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def files_list(self):
		return([f[0] for f in self.local_pkg.files] if self.local_pkg is not None else [])

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.pkg = pkg

	#-----------------------------------
	# Helper functions
	#-----------------------------------
	def int_to_datestr_short(self, value):
		return(datetime.datetime.fromtimestamp(value).strftime("%Y/%m/%d %H:%M") if value != 0 else "")

	def int_to_datestr_long(self, value):
		return(datetime.datetime.fromtimestamp(value).strftime("%a %d %b %Y %H:%M:%S") if value != 0 else "")

	def int_to_sizestr(self, value):
		pkg_size = value

		for unit in ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']:
			if pkg_size < 1024.0 or unit == 'PiB':
				break
			pkg_size /= 1024.0
		
		return(f"{pkg_size:.1f} {unit}")

	def url_to_link(self, url):
		return(f'<a href="{url}">{url}</a>')

	def pkglist_to_linkstr(self, pkglist):
		def link(string):
			pkg = string
			desc = ""
			ver = ""

			if ':' in pkg:
				pkg, desc = pkg.split(':', 1)
				desc = ':'+desc

			for delim in ['>=', '<=', '=', '>', '<']:
				if delim in pkg:
					pkg, ver = pkg.split(delim, 1)
					ver = delim+ver
					break

			pkg = GLib.markup_escape_text(pkg)
			desc = GLib.markup_escape_text(desc)
			ver = GLib.markup_escape_text(ver)

			return(f'<a href="pkg://{pkg}">{pkg}</a>{ver}{desc}')

		return('   '.join([link(s) for s in sorted(pkglist)]) if pkglist != [] else "None")

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

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, name, value, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.prop_name = name
		self.prop_value = value
