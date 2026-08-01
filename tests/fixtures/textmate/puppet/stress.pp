/*
Puppet grammar stress manifest for a small observatory service.
Human labels include café, Ελληνικά, 東京, and astral 🛰️ 🚀.
The declarations intentionally exercise resources, data, and orchestration.
*/

type Profile::Mode = Enum['active', 'passive', 'maintenance']
type Profile::Port = Integer[1, 65535]

class profile::base(
  String $owner = 'root',
  String $group = 'root',
  String $root = '/srv/observatory',
) {
  file { $root:
    ensure => directory,
    owner  => $owner,
    group  => $group,
    mode   => '0755',
  }

  file { "${root}/README.txt":
    ensure  => present,
    owner   => $owner,
    content => 'Managed by Puppet; interpolation stays literal: ${owner}',
  }
}

define profile::endpoint(
  Profile::Port $port,
  String $address = '127.0.0.1',
  Boolean $tls = false,
  Optional[String] $certificate = undef,
) {
  $scheme = $tls ? {
    true    => 'https',
    false   => 'http',
    default => 'http',
  }

  notify { "endpoint-${title}":
    message => "${scheme}://${address}:${port}",
    tag     => ['observatory', 'endpoint'],
  }

  if ($tls) {
    file { "/etc/observatory/certs/${title}.pem":
      ensure => file,
      source => $certificate,
      mode   => '0640',
    }
  }
}

function profile::qualified_label(
  String $name,
  Integer $sequence = 1,
) >> String {
  $clean = regsubst($name, /[^0-9A-Za-z_-]+/, '-', 'G')
  String("${clean}-${sequence}")
}

class profile::observatory(
  Profile::Mode $mode = 'active',
  Profile::Port $port = 8042,
  Array[String] $operators = ['Ada', 'Grace', 'Renée'],
  Hash[String, String] $labels = {
    site   => '東京',
    symbol => 'λ',
    craft  => '🛰️',
  },
  Boolean $manage_package = true,
) inherits profile::base {
  $_private_note = 'internal'
  $hex_mask = 0x2a
  $ratio = 3.1415
  $small = -12
  $large = +6.02e23
  $packages = ['nginx', 'curl', 'jq']
  $paths = {
    config => '/etc/observatory/observatory.conf',
    data   => '/var/lib/observatory',
    log    => '/var/log/observatory',
  }

  if ($manage_package and $mode != 'maintenance') {
    package { $packages:
      ensure => installed,
    }
  } elsif ($mode == 'maintenance') {
    notice('packages retained during maintenance')
  } else {
    debug('package management disabled')
  }

  unless ($operators.empty) {
    notify { 'operators-present':
      message => "Operators: ${operators}",
    }
  }

  case $mode {
    'active': {
      $service_ensure = running
      $service_enable = true
    }
    'passive': {
      $service_ensure = stopped
      $service_enable = false
    }
    default: {
      $service_ensure = stopped
      $service_enable = false
    }
  }

  $config = @("OBSERVATORY"/L)
    # Generated configuration for ${labels['site']} 🛰️
    mode=${mode}
    port=${port}
    owner=${owner}
    operators=${operators}
    escaped-dollar=\$HOME
    | OBSERVATORY

  $query = @(SQL)
    SELECT name, status
      FROM observations
     WHERE status = 'ready';
    | SQL

  file { ['/etc/observatory', $paths['data'], $paths['log']]:
    ensure => directory,
    owner  => $owner,
    group  => $group,
    mode   => '0750',
  }

  file { $paths['config']:
    ensure  => file,
    owner   => $owner,
    group   => $group,
    mode    => '0640',
    content => $config,
    require => Package['nginx'],
    notify  => Service['observatory'],
  }

  file { '/usr/local/bin/observatory-check':
    ensure  => file,
    mode    => '0755',
    content => "#!/bin/sh\ncurl -fsS http://127.0.0.1:${port}/health\n",
  }

  service { 'observatory':
    ensure     => $service_ensure,
    enable     => $service_enable,
    hasstatus  => true,
    hasrestart => true,
    subscribe  => File[$paths['config']],
  }

  profile::endpoint { 'public':
    port        => $port,
    address     => '0.0.0.0',
    tls         => true,
    certificate => 'puppet:///modules/profile/public.pem',
  }

  Package['nginx'] -> File[$paths['config']] ~> Service['observatory']
  tag('observatory', $mode)
}

class profile::diagnostics {
  $hostname = $facts['networking']['hostname']

  if $hostname =~ /^(web|api)-(\d+)$/ {
    $role = $1
    $ordinal = $2
    info("matched ${role} node ${ordinal}")
  } else {
    warning("unmatched hostname ${hostname}")
  }

  $summary = join(
    ['host', $hostname, 'kernel', $facts['kernel']],
    ':',
  )
  notice($summary) # trailing comment form
  alert('diagnostic alert sample')
  crit('diagnostic critical sample')
  err('diagnostic error sample')
}

plan profile::deploy(
  TargetSpec $targets,
  String $release = 'stable',
  Boolean $dry_run = false,
) {
  $nodes = get_targets($targets)
  if ($dry_run) {
    notice("Would deploy ${release} to ${nodes}")
  } else {
    run_task('package', $nodes, action => 'status')
    run_plan('profile::verify', targets => $nodes)
  }
}

node /^web\d+\.example\.test$/ {
  include profile::observatory
  contain profile::diagnostics
}

node 'archive.example.test' {
  class { 'profile::observatory':
    mode           => 'passive',
    port           => 9042,
    manage_package => false,
  }
}

node default {
  require profile::base
  realize(File['/srv/observatory'])
}
