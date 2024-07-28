#!/usr/bin/env bash

#==========================================================================================
# SCRIPT VARIABLES
#==========================================================================================

SCRIPTFOLDER=$(dirname $(realpath $0))

#==========================================================================================
# HELPER FUNCTIONS
#==========================================================================================
GREEN=$(tput setaf 10)
# BLUE=$(tput setaf 4)
# YELLOW=$(tput setaf 11)
# RED=$(tput setaf 9)
BOLD=$(tput bold)
RESET=$(tput sgr0)

print_item()
{
	echo -e "\n${GREEN}${BOLD}==>${RESET} ${BOLD}$1${RESET}"
}

#==========================================================================================
# MAKE RELEASE
#==========================================================================================
make_release()
{
	echo " "
	echo "--------------------------------------------------------------------------"
	echo "-- ${GREEN}MAKE RELEASE${RESET}"
	echo "--------------------------------------------------------------------------"

	local version=""
	local confirm="n"

	if [[ -z "$1" ]]; then
		echo ""
		read -e -p "Input release version: " version
	else
		version="$1"
	fi

	echo ""
	echo "Making release for version ${GREEN}${version}${RESET}"
	read -r -s -n 1 -p "Are you sure you want to continue [y/N]: " confirm
	echo ""

	if [[ "${confirm,,}" == "y" ]]; then
		print_item "Updating version in Cargo.toml"
		sed -i "/^version = / c version = \"${version}\"" "$SCRIPTFOLDER/Cargo.toml"

		print_item "Updating version in app.rs"
		sed -i "/.version/ c \            .version(\"${version}\")" "$SCRIPTFOLDER/src/app.rs"

		print_item "Building release version"
		cargo build --release

		print_item "Copying release version executable"
		cp "$SCRIPTFOLDER/target/release/pacview" "$SCRIPTFOLDER"

		print_item "Committing git changes"
		git add "$SCRIPTFOLDER/Cargo.toml"
		git add "$SCRIPTFOLDER/Cargo.lock"
		git add "$SCRIPTFOLDER/src/app.rs"
		git add "$SCRIPTFOLDER/pacview"
		git commit -m "Bump version to ${version}"

		print_item "Adding git tag ($version)"
		git tag "$version"

		print_item "Pushing changes to upstream repository"
		git push
		git push origin --tags
	fi
}

make_release "$1"
