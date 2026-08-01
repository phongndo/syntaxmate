/* JSX stress fixture: café, λ, 東京, and 🚀 */
import React, { Fragment, useEffect, useMemo, useState } from "react";
import * as UI from "./ui-kit.js";
import defaultAvatar, { loadProfile as fetchProfile } from "./profiles.js";

const APP_NAME = "Mission Café";
const VERSION = 0x2a;
const MAX_RETRIES = 3;
const launchCode = 0b101101n;
const handlePattern = /^(?<name>[\p{L}\d_-]{2,24})$/iu;
const unsafeMarkup = /<script\b[^>]*>[\s\S]*?<\/script>/gi;
const css = String.raw;
const panelTheme = css`color: var(--ink, #223); grid-template-columns: repeat(${VERSION % 4}, 1fr);`;

const initialFilters = Object.freeze({ query: "", roles: ["pilot", "engineer"], active: true });

function joinClass(...parts) {
  return parts.filter(Boolean).join(" ");
}

function normalizeMember(member, index) {
  const { profile: rawProfile = {}, ...identity } = member ?? {};
  const profile = { avatar: defaultAvatar, timezone: "UTC", ...rawProfile };
  return {
    ...identity,
    id: member?.id ?? `guest-${index}`,
    name: member?.name?.trim() || "Anonymous",
    profile,
  };
}

async function requestRoster(endpoint, signal) {
  let attempt = 0;
  while (attempt++ < MAX_RETRIES) {
    try {
      const response = await fetch(`${endpoint}?include=profile`, { headers: { Accept: "application/json" }, signal });
      if (!response.ok) throw new Error(`Roster failed: ${response.status}`);
      return await response.json();
    } catch (error) {
      if (signal?.aborted || attempt >= MAX_RETRIES) throw error;
      await new Promise((resolve) => setTimeout(resolve, attempt * 25));
    }
  }
  return [];
}

const stateLabels = { ready: "Ready", away: "Away" };
const statusLabel = (state) => stateLabels[state] ?? "Unknown";

class RosterBoundary extends React.Component {
  state = { error: null };
  static getDerivedStateFromError(error) {
    return { error };
  }
  componentDidCatch(error, info) {
    console.error("roster boundary", error?.message, info.componentStack);
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <UI.Notice tone="danger" role="alert">
        <Fragment>
          <strong>Unable to render roster.</strong>
          <code>{this.state.error?.message ?? "unknown error"}</code>
        </Fragment>
      </UI.Notice>
    );
  }
}

const Avatar = ({ member, size = 32, ...imageProps }) => (
  <img
    {...imageProps}
    className={joinClass("avatar", imageProps.className)}
    src={member.profile?.avatar ?? defaultAvatar}
    alt={`${member.name}'s avatar`}
    width={size} height={size} loading="lazy"
  />
);

function MemberRow({ member, selected, onSelect }) {
  const badge = member.profile?.badges?.[0];
  return (
    <li className={joinClass("member", selected && "is-selected")} data-id={member.id}>
      <button type="button" aria-pressed={selected} onClick={() => onSelect(member.id)}>
        <Avatar member={member} size={40} draggable={false} />
        <span className="member__copy">
          <b>{member.name}</b>
          <small title={member.profile?.timezone}>{statusLabel(member.state)} &middot; {member.profile?.timezone ?? "UTC"}</small>
        </span>
        {badge ? <UI.Badge tone={badge.tone || "neutral"}>{badge.label}</UI.Badge> : null}
      </button>
    </li>
  );
}

function FilterBar({ filters, onChange }) {
  const toggleActive = () => onChange({ ...filters, active: !filters.active });
  return (
    <fieldset className="filters">
      <legend>Roster filters</legend>
      <label htmlFor="member-query">Search</label>
      <input
        id="member-query" name="query"
        value={filters.query} placeholder={'Try "Ada" or λ'}
        onChange={(event) => onChange({ ...filters, query: event.currentTarget.value })}
      />
      <label>
        <input type="checkbox" checked={filters.active} onChange={toggleActive} />
        Active only
      </label>
    </fieldset>
  );
}

export function MissionRoster({ endpoint = "/api/crew", seed = [], onLaunch }) {
  const [members, setMembers] = useState(() => seed.map(normalizeMember));
  const [filters, setFilters] = useState(initialFilters);
  const [selectedId, setSelectedId] = useState(null);

  useEffect(() => {
    const controller = new AbortController();
    requestRoster(endpoint, controller.signal)
      .then((rows) => setMembers(rows.map(normalizeMember)))
      .catch((error) => error?.name !== "AbortError" && console.warn(error));
    return () => controller.abort();
  }, [endpoint]);

  const visibleMembers = useMemo(() => {
    const needle = filters.query.toLocaleLowerCase();
    return members
      .filter((member) => !filters.active || member.active !== false)
      .filter((member) => member.name.toLocaleLowerCase().includes(needle))
      .sort((left, right) => left.name.localeCompare(right.name));
  }, [filters, members]);

  const selected = members.find(({ id }) => id === selectedId);
  const canLaunch = Boolean(selected?.active && handlePattern.test(selected.name));
  const launch = () => onLaunch?.({ member: selected, code: launchCode });

  return (
    <RosterBoundary>
      <UI.Card className="mission-roster" style={{ "--panel-theme": panelTheme }}>
        <UI.Card.Header>
          <>
            <h1>{APP_NAME} <span aria-hidden="true">🚀</span></h1>
            <p>{`Crew count: ${visibleMembers.length}`}</p>
          </>
        </UI.Card.Header>
        <UI.Card.Body>
          <FilterBar filters={filters} onChange={setFilters} />
          {/* Expressions, comments, fragments, and nested component tags stay balanced. */}
          {visibleMembers.length > 0 ? (
            <ul aria-label="Mission crew">
              {visibleMembers.map((member) => (
                <MemberRow
                  key={member.id}
                  member={member}
                  selected={member.id === selectedId}
                  onSelect={setSelectedId}
                />
              ))}
            </ul>
          ) : (
            <UI.EmptyState icon="search">
              No crew match <q>{filters.query || "all"}</q>.
            </UI.EmptyState>
          )}
        </UI.Card.Body>
        <UI.Card.Footer data-version={VERSION} hidden={!selected}>
          <button type="button" disabled={!canLaunch} onClick={launch}>
            Launch with {selected?.name ?? "a crew member"}
          </button>
        </UI.Card.Footer>
      </UI.Card>
    </RosterBoundary>
  );
}

export async function preloadProfile(id) {
  const module = await import("./profile-cache.js");
  const cached = module.default?.get?.(id);
  return cached ?? fetchProfile(id).then((profile) => ({ ...profile, bio: profile.bio?.replace(unsafeMarkup, "") }));
}

export default MissionRoster;
