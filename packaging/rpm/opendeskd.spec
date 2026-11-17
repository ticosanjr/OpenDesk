Name:           opendeskd
Version:        %{version}
Release:        1
Summary:        OpenDesk headless control plane
License:        MIT
URL:            https://github.com/ticosanjr/OpenDesk
Requires:       systemd

%description
Headless OpenDesk daemon. It serves the local API and exposes the
opendeskd systemd user unit. The desktop package is separate.

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/lib/systemd/user
install -m 0755 %{_sourcedir}/opendeskd %{buildroot}/usr/bin/opendeskd
install -m 0644 %{_sourcedir}/opendeskd.service %{buildroot}/usr/lib/systemd/user/opendeskd.service

%files
/usr/bin/opendeskd
/usr/lib/systemd/user/opendeskd.service

%changelog
* Tue Nov 17 2026 OpenDesk <opendesk@localhost> - %{version}-1
- Headless daemon and systemd user unit.
