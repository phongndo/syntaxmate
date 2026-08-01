{{-- Compact Blade coverage: café, 日本語, and astral 🚀 𝌆. --}}
@php($title = 'Mission board')
<!doctype html>
<html lang="{{ app()->getLocale() }}">
<head>
  <meta charset="utf-8">
  <title>{{ $title }}</title>
</head>
<body @class(['theme-dark' => $dark, 'theme-light' => ! $dark])>
  <h1>{{ $title }}</h1>
  @if ($user)
    <p>Hello, {{ $user->name }}!</p>
  @else
    <p>Welcome, traveler 🚀.</p>
  @endif
  <ul>
    @forelse ($missions as $mission)
      <li data-id="{{ $mission->id }}">{{ $mission->name }}</li>
    @empty
      <li>No missions for 日本語 readers.</li>
    @endforelse
  </ul>
  <div class="trusted">{!! $trustedSummary !!}</div>
  <code>@{{ clientSideValue }}</code>
</body>
</html>
