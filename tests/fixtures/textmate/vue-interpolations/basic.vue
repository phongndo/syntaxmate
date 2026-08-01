<template>
  <section class="launch-card" aria-labelledby="launch-title">
    <h1 id="launch-title">{{ dashboard.title }}</h1>
    <p>{{ greeting ?? 'café 東京 λ' }}</p>
    <p>{{ online ? `Ready for ${operator.name} 🚀` : "Waiting 𝌆" }}</p>
    <p>{{ completed + 1 }} of {{ tasks.length }} checks</p>
    <p>{{ user?.profile?.label ?? 'Guest operator' }}</p>
    <p>{{ Math.round(progress * 100) }}% complete</p>
    <p>{{ selectedIds.includes(activeTask.id) && !activeTask.archived }}</p>
    <output>{{ JSON.stringify({ ready, count: tasks.length }) }}</output>
    <p>{{ formatDate(lastSync, { dateStyle: 'medium', timeStyle: 'short' }) }}</p>
    <p>{{ labels.filter((label) => label.visible).map(({ name }) => name).join(', ') }}</p>
    <p>{{ /^ops-[a-z]+$/i.test(team.slug) ? team.slug : 'unassigned' }}</p>
    <footer>
      {{ account.plan === 'pro' ? `${quota.used}/${quota.limit}` : 'Upgrade required' }}
    </footer>
  </section>
</template>
