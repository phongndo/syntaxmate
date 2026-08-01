import { computed, ref } from 'vue'

type GreetingProps = { name: string; excited?: boolean }

export function GreetingCard(props: GreetingProps) {
  const visits = ref(0);
  const title = computed(() => props.name + ' — café 🚀 𝌆');
  vineStyle.scoped(css`
    .greeting-card {
      padding: 1rem;
      color: rebeccapurple;
    }
  `);
  return vine`
    <article class="greeting-card" :data-visits="visits">
      <!-- 日本語の挨拶 🚀 -->
      <h1>{{ title }}</h1>
      <button type="button" @click="visits++">
        Visits: {{ visits }}
      </button>
    </article>
  `;
}
