{{--
  A reviewable Blade dashboard covering directives, PHP islands, and host HTML.
  Unicode includes café, Ελληνικά, 日本語, 🚀, and the astral symbol 𝌆.
--}}
@extends('layouts.app')
@inject('formatter', 'App\Support\MissionFormatter')
@php
    /** Prepare ordinary PHP values before rendering the view. */
    $limits = ['warning' => 70, 'critical' => 90];
    $hexMask = 0x2A;
    $binaryMask = 0b1010;
    $octalMode = 0755;
    $ratio = 6.25e-2;
    $label = "Mission {$mission->name} — 🚀";
    $escaped = 'café \'quoted\' path\\segment';
    $scores = array_map(function ($value) use ($ratio) {
        return round($value * $ratio, 2);
    }, [10, 20, 30]);
    $welcome = <<<TEXT
Welcome to {$mission->name}.
Telemetry is available in 日本語 and Ελληνικά.
TEXT;
    $schema = <<<'JSON'
{"kind":"mission","glyph":"𝌆","enabled":true}
JSON;
@endphp
@section('title', $label)
@push('styles')
<style>
  :root { --accent: {{ $theme['accent'] ?? '#5b4bdb' }}; }
  .dashboard { display: grid; gap: 1rem; }
  .metric[data-state="critical"] { border-inline-start: .25rem solid crimson; }
  @media (min-width: 48rem) {
    .dashboard { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }
</style>
@endpush
@prepend('scripts')
<script>window.viewStartedAt = Date.now();</script>
@endprepend
@section('content')
<main id="mission-{{ $mission->id }}" @class([
  'dashboard',
  'dashboard--compact' => $compact,
  'dashboard--readonly' => ! $canEdit,
]) data-mask="{{ $hexMask }}" :data-mission="$mission">
  <header>
    <p class="eyebrow">{{ __('Mission control') }}</p>
    <h1>{{ $formatter->title($mission) }}</h1>
    <p>{{ $welcome }}</p>
    <small>{{ $schema }} · ratio {{ $ratio }}</small>
  </header>
  @auth
    <p>Signed in as {{ auth()->user()->name }}.</p>
  @else
    <a href="{{ route('login') }}">Sign in</a>
  @endauth

  @can('update', $mission)
    <a href="{{ route('missions.edit', $mission) }}">Edit mission</a>
  @elsecan('viewAudit', $mission)
    <a href="{{ route('missions.audit', $mission) }}">View audit</a>
  @else
    <span>Read-only access</span>
  @endcan

  @unless ($mission->archived)
    <p role="status">Live telemetry is enabled.</p>
  @endunless

  @isset($mission->summary)
    <section class="summary">{{ $mission->summary }}</section>
  @endisset

  @empty($mission->owner)
    <p class="notice">No owner has been assigned.</p>
  @endempty

  @switch($mission->status)
    @case('ready')
      <span class="badge badge--ready">Launch ready</span>
      @break
    @case('delayed')
      <span class="badge badge--warning">Delayed</span>
      @break
    @default
      <span class="badge">Planning</span>
  @endswitch

  <section aria-labelledby="metric-heading">
    <h2 id="metric-heading">Metrics</h2>
    <div class="metrics">
      @foreach ($metrics as $key => $metric)
        @continue($metric['hidden'] ?? false)
        <article @class(['metric', 'metric--first' => $loop->first])
                 @style(['opacity: .65' => $metric['stale']])
                 data-state="{{ $metric['value'] >= $limits['critical'] ? 'critical' : 'normal' }}">
          <h3>{{ Str::headline($key) }}</h3>
          <data value="{{ $metric['value'] }}">{{ number_format($metric['value'], 2) }}</data>
          @if ($metric['value'] >= $limits['critical'])
            <strong>Critical</strong>
          @elseif ($metric['value'] >= $limits['warning'])
            <em>Warning</em>
          @else
            <span>Nominal</span>
          @endif
        </article>
      @endforeach
    </div>
  </section>

  <ol class="checklist">
    @forelse ($mission->checks as $check)
      <li>
        <input type="checkbox" name="checks[]" value="{{ $check->id }}"
               @checked($check->complete) @disabled(! $canEdit)>
        <span>{{ $check->label }}</span>
        @if ($loop->remaining === 0)<small>Final check</small>@endif
      </li>
    @empty
      <li>No checks configured for café operations.</li>
    @endforelse
  </ol>

  <form method="POST" action="{{ route('missions.update', $mission) }}">
    @csrf
    @method('PATCH')
    <label for="region">Region</label>
    <select id="region" name="region" @required($canEdit)>
      @foreach ($regions as $code => $name)
        <option value="{{ $code }}" @selected($mission->region === $code)>{{ $name }}</option>
      @endforeach
    </select>
    <label for="notes">Notes</label>
    <textarea id="notes" name="notes" @readonly(! $canEdit)>{{ old('notes', $mission->notes) }}</textarea>
    @error('notes')
      <p class="error">{{ $message }}</p>
    @enderror
    <button type="submit" @disabled(! $canEdit)>Save</button>
  </form>

  @component('components.panel', ['tone' => 'info'])
    @slot('title')
      Flight plan
    @endslot
    <p>{{ $mission->flightPlan->summary }}</p>
  @endcomponent

  <x-mission-card :mission="$mission" :scores="$scores">
    <x-slot:heading>{{ $label }}</x-slot:heading>
    <p>{!! $trustedMissionHtml !!}</p>
  </x-mission-card>

  @includeWhen($showHistory, 'missions.partials.history', ['mission' => $mission])
  @includeIf('missions.partials.' . $mission->status)
  @includeFirst(['missions.partials.custom', 'missions.partials.fallback'])

  @once
    <template id="mission-toast"><p>Mission updated.</p></template>
  @endonce

  @env(['local', 'testing'])
    <pre>{{ json_encode($debug, JSON_PRETTY_PRINT) }}</pre>
  @endenv

  @production
    <p class="production-note">Production telemetry is monitored.</p>
  @endproduction

  @verbatim
    <template id="client-row">
      <span>{{ client.name }}</span>
      <strong>@{{ client.status }}</strong>
    </template>
  @endverbatim

  <script type="application/json" id="mission-data">
    @json(['id' => $mission->id, 'name' => $mission->name, 'scores' => $scores])
  </script>
  <script>
    const mission = {{ Js::from($mission->only(['id', 'name', 'status'])) }};
    const root = document.querySelector(`#mission-${mission.id}`);
    root?.addEventListener('click', (event) => console.debug(event.target));
  </script>

  <?php $legacyStatus = $mission->status ?: 'planning'; ?>
  <footer data-status="{{ $legacyStatus }}">
    <span>{{{ $escaped }}}</span>
    <strong>{!! nl2br(e($mission->footer)) !!}</strong>
  </footer>
</main>
@endsection

@pushOnce('scripts')
<script src="{{ vite_asset('resources/js/mission.js') }}" defer></script>
@endPushOnce

@customTelemetry($mission->id, ['mask' => $binaryMask, 'mode' => $octalMode])
@stack('scripts')
