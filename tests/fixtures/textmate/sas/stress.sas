/*
  Regional order pipeline used to exercise a broad SAS program.
  Unicode samples are intentional: BMP Ω, Ж, 中; astral 😀, 𐐷, 𝄞.
*/
options mprint mlogic symbolgen nodate nonumber;
libname stage "/tmp/sas-stage";
filename orders "/tmp/orders.csv" encoding="utf-8";

%let report_day = '31MAR2025'd;
%let generated_at = "31MAR2025:23:59:59"dt;
%let minimum_total = 25.50;
%let regions = North South East West;

* This statement comment begins on one line,
  remains active on the next, and closes here;

%macro import_region(region=, code=);
  data stage.orders_&code.(label="Orders for &region.");
    length order_id 8 customer $40 region $12 item $32 note $80;
    infile orders dsd dlm=',' firstobs=2 truncover lrecl=512;
    input order_id customer :$40. region :$12. item :$32.
          quantity unit_price ordered_on :date9. note :$80.;
    if upcase(region) = upcase("&region.");
    gross = quantity * unit_price;
    tax = round(gross * 0.075, 0.01);
    total = sum(gross, tax);
    if total >= &minimum_total then priority = 'Y';
    else priority = 'N';
    source_code = "&code.";
    format ordered_on date9. gross tax total dollar10.2;
    label total="Invoice total" note="Unicode note Ω 中 😀";
  run;
%mend import_region;

%import_region(region=North, code=N)
%import_region(region=South, code=S)
%import_region(region=East, code=E)
%import_region(region=West, code=W)

data work.manual_orders;
  length customer $40 region $12 item $32 note $80;
  infile datalines dsd dlm='|';
  input order_id customer $ region $ item $ quantity unit_price
        ordered_on :date9. note $;
  datalines;
9001|Zoë|North|Notebook|2|12.50|01MAR2025|BMP Ω and 中
9002|Miyuki|East|Pen set|4|8.75|03MAR2025|Astral 😀 and 𐐷
9003|Renée|West|Music paper|3|6.25|04MAR2025|Clef 𝄞
;
run;

data work.all_orders(compress=yes);
  set stage.orders_n(in=from_n)
      stage.orders_s(in=from_s)
      stage.orders_e(in=from_e)
      stage.orders_w(in=from_w)
      work.manual_orders(in=from_manual);
  if from_manual then source_code = 'M';
  else if from_n then source_code = 'N';
  else if from_s then source_code = 'S';
  else if from_e then source_code = 'E';
  else if from_w then source_code = 'W';
  gross = quantity * unit_price;
  tax = round(gross * 0.075, 0.01);
  total = gross + tax;
  loaded_at = &generated_at;
  format loaded_at datetime20.;
run;

proc sort data=work.all_orders
          out=work.sorted_orders nodupkey;
  by order_id;
run;

data work.valid_orders(drop=bad_reason)
     work.rejected_orders(keep=order_id customer bad_reason);
  set work.sorted_orders;
  length bad_reason $60;
  if missing(customer) then bad_reason = 'Missing customer';
  else if quantity <= 0 then bad_reason = 'Nonpositive quantity';
  else if unit_price < 0 then bad_reason = 'Negative price';
  if bad_reason ne '' then output work.rejected_orders;
  else output work.valid_orders;
run;

data work.enriched(rename=(customer=customer_name));
  set work.valid_orders(
    where=(ordered_on <= &report_day)
    keep=order_id customer region item quantity unit_price ordered_on note total
  );
  array amount_parts[3] unit_price total discount;
  discount = 0;
  do index = 1 to dim(amount_parts);
    if amount_parts[index] = . then amount_parts[index] = 0;
  end;
  select (upcase(region));
    when ('NORTH') shipping = 5;
    when ('SOUTH') shipping = 7;
    when ('EAST')  shipping = 6;
    when ('WEST')  shipping = 8;
    otherwise shipping = 10;
  end;
  net_total = total - discount + shipping;
  format net_total dollar10.2;
run;

proc format;
  value total_band
    low-<50 = 'small'
    50-<200 = 'medium'
    200-high = 'large';
  value $region_name
    'NORTH' = 'Northern Ω'
    'SOUTH' = 'Southern Ж'
    'EAST'  = 'Eastern 中'
    'WEST'  = 'Western 😀';
run;

data work.customer_flags;
  update work.enriched(obs=100)
         work.enriched(firstobs=2 obs=100);
  by order_id;
  retained_total + net_total;
  first_letter = substr(customer_name, 1, 1);
  normalized = propcase(compbl(customer_name));
  has_unicode = prxmatch('/[^\x00-\x7F]/', note) > 0;
  format net_total total_band.;
run;

data work.catalog_changes;
  modify work.customer_flags;
  if net_total > 500 then do;
    priority = 'Y';
    replace;
  end;
run;

data work.region_lookup;
  length region $12 manager $40;
  input region $ manager & $40.;
  cards;
North Ada Lovelace
South Grace Hopper
East Katherine Johnson
West Alan Turing
;
run;

data work.with_manager;
  merge work.enriched(in=has_order)
        work.region_lookup(in=has_manager);
  by region;
  if has_order;
  manager_known = has_manager;
run;

proc sql;
  create table work.region_summary as
  select upcase(region) as region_key length=12,
         count(distinct order_id) as order_count,
         sum(quantity) as units,
         sum(net_total) as revenue format=dollar12.2,
         mean(net_total) as average_order format=dollar10.2
  from work.enriched
  where ordered_on between '01JAN2025'd and &report_day
    and region is not null
  group by calculated region_key
  having calculated revenue >= &minimum_total
  order by calculated revenue desc;

  create view work.priority_orders as
  select a.order_id, a.customer_name, a.region, a.net_total,
         b.manager
  from work.enriched as a
  left join work.region_lookup as b
    on upcase(a.region) = upcase(b.region)
  where a.net_total > 100;
quit;

proc means data=work.enriched n mean median min max maxdec=2;
  class region;
  var quantity unit_price net_total;
  output out=work.region_stats mean= p50= / autoname;
run;

proc freq data=work.enriched order=freq;
  tables region*priority / missing norow nocol;
run;

proc transpose data=work.region_summary
               out=work.revenue_wide prefix=revenue_;
  id region_key;
  var revenue;
run;

ods html path="/tmp" file="orders.html" style=HTMLBlue;
title1 "Quarterly order report — Ω 中 😀";
footnote1 "Generated &report_day; build 𐐷";
proc report data=work.priority_orders nowd;
  columns region customer_name manager net_total;
  define region / group 'Region';
  define customer_name / display 'Customer';
  define manager / display 'Manager';
  define net_total / analysis sum format=dollar12.2 'Revenue';
  break after region / summarize;
run;
ods html close;

%macro publish(enabled=YES);
  %if %upcase(&enabled.) = YES %then %do;
    proc print data=work.region_summary noobs label;
      var region_key order_count units revenue average_order;
    run;
  %end;
  %else %do;
    %put NOTE: Publishing was disabled.;
  %end;
%mend publish;

%publish(enabled=YES)

/* Final multiline comment verifies that comment state closes cleanly.
   Every DATA, PROC, SQL, macro, quote, and comment above is terminated. */
title;
footnote;
