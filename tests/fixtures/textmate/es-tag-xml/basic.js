const label = "host café 東京 🚀 𝌆";

const feed = xml`<?xml version="1.0" encoding="UTF-8"?>
  <feed xmlns="urn:example:feed" xml:lang="ja">
    <!-- complete XML comment -->
    <title>café 東京 🚀 𝌆</title>
    <entry id="one">
      <summary>Tea &amp; coffee</summary>
    </entry>
  </feed>
`;

const icon = /* svg */ `
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
    <circle cx="8" cy="8" r="7" />
  </svg>
`;

const hostResult = { label, feed, icon, resumed: true };
export default hostResult;
