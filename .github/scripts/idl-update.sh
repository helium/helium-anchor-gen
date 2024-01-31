#!/bin/bash

OWNER=lthiery
REPO=helium-program-library

function version_from_tag() {
  TAG=$1
  PREFIX=$2
  # extract the version from the name (eg program-data-credits-0.2.1 -> 0.2.1)
  VERSION=$(echo "$TAG" | sed -e "s/^$PREFIX-//")
  echo "$VERSION"
}

function get_tags() {
  PROGRAM=$1
  PAGE=$2
  URL="https://api.github.com/repos/${OWNER}/${REPO}/tags?page=$PAGE"
  # sort alphabetically, reverse, and filter by program name
  TAGS=$(curl -s -L \
    -H "Accept: application/vnd.github+json" \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "$URL")


  # check if curl command failed
  status_code=$?
  if [ $status_code -ne 0 ]; then
    echo "${TAGS}"
    exit 1
  fi

  LIST=$(echo "$TAGS" | jq -r --arg P "$PROGRAM" '.[].name | select(startswith($P))')
  # check jq was successful
  status_code=$?
  if [ $status_code -ne 0 ]; then
    echo "${LIST} ${TAGS}"
    exit 1
  fi


  # protect against the case where there is only one tag and it contains a 'v' prefix to the version
  if [ $(echo "$LIST" | wc -l) -eq 1 ]; then
    # replace underscores with hyphens
    PREFIX=program-${P//_/-}
    # get version from the first element of the list
    VERSION=$(version_from_tag "$LIST" "PREFIX")
    # if the version contains a v prefix, return an empty string
    if [[ "$VERSION" =~ ^v.* ]]; then
      echo ""
      exit 0
    fi
  fi

  # in case any of the tags have a 'v' prefix, remove it
  for ((i=0; i<${#LIST[@]}; i++)); do
    LIST[$i]=$(echo "${LIST[$i]}" | sed 's/^\(.*-\)v\(.*\)$/\1\2/')
  done
  # sort the list and return the top
  echo "${LIST[@]}" | tr ' ' '\n' | sort -r | head -n 1
}

function update_idl() {
  PROGRAM=$1
  TAG=$2
  echo "updating idl $PROGRAM to $TAG"
  # download the json idl file from the tags assets
  URL="https://github.com/${OWNER}/${REPO}/releases/download/${TAG}/${PROGRAM}.json"

  curl -L \
    -o "idl/${PROGRAM}.json.tmp" \
    -H "Accept: application/vnd.github+json" \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "$URL"
  status_code=$?

  # if curl failed, exit
  if [ $status_code -ne 0 ]; then
    echo "Download failed"
    exit 1
  fi
  # otherwise, rename file
  mv "idl/${PROGRAM}.json.tmp" "idl/${PROGRAM}.json"

  # replace underscores with hyphens
  PREFIX=program-${P//_/-}
  # write the new tag to the Cargo.toml
  VERSION=$(version_from_tag "$TAG" "$PREFIX")
  PROGRAM_DIR=${PROGRAM//_/-}
  sed -i -e "s/^version = \".*\"/version = \"${VERSION}\"/" "programs/$PROGRAM_DIR/Cargo.toml"
  exit 0
}

function get_cargo_version() {
  PROGRAM=$1
  PROGRAM_DIR=${PROGRAM//_/-}
  # get the version from the Cargo.toml file
  CARGO_VERSION=$(grep -m 1 "version" "programs/$PROGRAM_DIR/Cargo.toml" | sed -e "s/version = \"//" -e "s/\"//")
  echo "$CARGO_VERSION"
}

function random_delay() {
  # Generate a random integer between 10 and 3276
  random_int=$((RANDOM % (3276 - 10 + 1) + 10))
  # Convert the random integer to a floating-point number between 0.1 and 1
  random_float=$(echo "scale=1; $random_int / 3276" | bc)
  sleep $random_float
}

function check_rate_limit() {
  RESPONSE=$(curl -s -L \
    -H "Accept: application/vnd.github+json" \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    https://api.github.com/rate_limit)

    REMAINING=$(echo "$RESPONSE" | jq -r '.rate.remaining')
    if [ "$REMAINING" -eq 0 ]; then
      RESET_TIME=$(echo "$RESPONSE" | jq -r '.rate.reset')
      echo "Rate limit exceeded, reset at $RESET_TIME"
      exit 1
    fi
}

P=$1
# replace underscores with hyphens
PREFIX=program-${P//_/-}
PAGE=1
while true; do
  # bail out early if we are rate limited.
  check_rate_limit
  status_code=$?
  if [ $status_code -ne 0 ]; then
    exit 1
  fi

  TAG=$(get_tags "$PREFIX" "$PAGE")
  random_delay
  #if tag exited with non-zero, exit
  status_code=$?
  if [ $status_code -ne 0 ]; then
    echo "Error getting tags"
    echo "${TAG}"
    exit 1
  fi

  # the first page of tags with the tag will be the latest
  # since the tags endpoint sorts alphabetically
  if [ -n "$TAG" ]; then
    # extract the version from the name (eg program-data-credits-0.2.1 -> 0.2.1)
    VERSION=$(version_from_tag "$TAG" "$PREFIX")
    CARGO_VERSION=$(get_cargo_version "$P")
    echo "Latest version of $P is $VERSION and Cargo.toml version is $CARGO_VERSION"
    # if VERSION and CARGO_VERSION are different, update the JSON
    if [ "$VERSION" != "$CARGO_VERSION" ]; then
      update_idl "${P}" "${TAG}"
    fi
    break
  fi
  # if tag is not found, increment page
  PAGE=$((PAGE+1))
done
