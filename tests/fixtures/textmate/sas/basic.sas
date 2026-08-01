/* Small sales summary; BMP: Ω 中, astral: 😀 𐐷. */
%let cutoff = '01JAN2025'd;
data work.recent(label="Unicode Ω 中 😀");
  set sashelp.class(keep=name age height weight);
  where age >= 13;
  bmi = (weight / (height * height)) * 703;
  if bmi > 20 then band = "high";
  else band = 'normal';
  format bmi 6.2;
run;

* A statement comment that closes at its semicolon;
proc sql;
  create table work.summary as
  select band, count(*) as pupils, mean(bmi) as mean_bmi
  from work.recent
  group by band
  order by mean_bmi desc;
quit;

proc print data=work.summary noobs;
  title "Résumé Ω — pupils 😀";
run;
