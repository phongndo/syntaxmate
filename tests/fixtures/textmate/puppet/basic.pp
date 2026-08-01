# Compact Puppet manifest: café, 東京, and deployment 🚀.
class profile::welcome(
  String $owner = 'Ada',
  Array[String] $locales = ['en', '日本語'],
  Boolean $enabled = true,
) {
  $message = "Hello ${owner} — café 🚀"
  $settings = { owner => $owner, locales => $locales }

  file { '/etc/motd':
    ensure  => file,
    owner   => 'root',
    content => "${message}\n",
  }

  if ($enabled) {
    notify { 'welcome-ready': message => $message }
  } else {
    warning('welcome disabled')
  }
}

include profile::welcome
