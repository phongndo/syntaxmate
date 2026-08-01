@Component({
  selector: 'app-card',
  styles: [`
    $accent: rebeccapurple;
    .card--東京 {
      color: $accent;
      content: "café 🚀 𝌆";
      &:hover { transform: translateX(1px); }
    }
  `],
  template: '<article>host text</article>',
})
export class CardComponent {}

const metadata = {
  styles: ('button { color: red; }'),
  other: true,
}

styles: '.standalone { display: grid; }', other: true
