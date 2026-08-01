# ROS interface TextMate stress fixture
# Grammar-driven coverage: types, defaults, arrays, strings, and separators.
# Unicode spans scripts and planes: naïve Ελληνικά 日本語 🚀
MSG: fixture_msgs/StressReport
@optional
std_msgs/Header header
bool ready true
bool halted FALSE
byte legacy_byte 7
char status_code 65
int8 delta8 -8
uint8 level8 +8
int16 delta16 -1600
uint16 level16 1600
int32 delta32 -320000
uint32 level32 320000
int64 delta64 -6400000000
uint64 level64 6400000000
float32 ratio 0.625
float32 offset -.25
float64 gain +12.5e-2
float64 whole_form 8.
time legacy_stamp
duration legacy_timeout
builtin_interfaces/Time stamp
builtin_interfaces/Duration timeout
geometry_msgs/Pose pose
geometry_msgs/PoseStamped target
sensor_msgs/Imu imu
fixture_msgs/Nested local_nested
fixture_msgs/msg/Qualified qualified_nested
# Fixed, bounded, and unbounded primitive arrays.
bool[] switches [true, FALSE, true]
bool[3] fixed_switches [true, false, true]
bool[<=8] bounded_switches [false, true]
byte[] bytes [0, 7, 255]
char[4] tag_bytes [82, 79, 83, 50]
int8[] signed_octets [-1, +2, -3]
uint8[4] rgba [0, 127, 255, 64]
int16[3] offsets16 [-32, 0, +32]
uint16[<=4] levels16 [1, 2, 3, 4]
int32[] offsets32 [-1_000, 0, 1_000]
uint32[2] masks32 [255, 65_535]
int64[<=3] offsets64 [-9, 0, +9]
uint64[] counters64 [1, 2, 3]
float32[] samples32 [.5, 1., -2.25]
float32[3] vector32 [+1.0, -2.0, 3.5e+1]
float64[36] covariance
float64[<=6] samples64 [0.0, -1.5E-2, +6.02e+2]
geometry_msgs/Point[] points
geometry_msgs/Quaternion[4] orientations
geometry_msgs/Twist[<=12] twists
# Scalar strings exercise quoted, unquoted, wide, bounded, and escapes.
string plain_topic /robot/status
string empty_text ""
string double_text "line\nquote\"slash\\"
string single_text 'tab\t and apostrophe\''
string octal_text "A=\101"
string hex_text "B=\x42"
string bmp_escape "snowman=\u2603"
string astral_escape "rocket=\U0001F680"
string unicode_text "crème brûlée 東京 🛰"
string<=16 bounded_text "short value"
wstring wide_text "Καλημέρα κόσμε"
wstring<=24 bounded_wide 'добрый день'
# String arrays cover each element form and string bounds.
string[] names [alpha, beta, gamma]
string[3] axes ["roll", 'pitch', yaw]
string[<=5] labels [one, "two words", 'three']
string<=12[] short_names [red, green, blue]
string<=16[4] fixed_short_names [north, south, east, west]
string<=20[<=3] bounded_short_names ["café", '東京', 🚀]
wstring[] wide_names ["α", 'β', gamma]
wstring<=12[<=3] bounded_wide_names [uno, dos, tres]
string[] escaped_names [line\nfeed, "hex\x21", 'tab\tstop']
# Constants exercise every primitive family recognized by the grammar.
bool FEATURE_ON=true
bool FEATURE_OFF=FALSE
byte BYTE_MARK=7
char ASCII_Z=90
int8 MINUS_EIGHT=-8
uint8 PLUS_EIGHT=+8
int16 NEGATIVE_SHORT=-16
uint16 POSITIVE_SHORT=16
int32 NEGATIVE_LONG=-32_000
uint32 POSITIVE_LONG=32_000
int64 NEGATIVE_WIDE=-64_000_000
uint64 POSITIVE_WIDE=64_000_000
float32 HALF=.5
float32 WHOLE=2.
float64 SCIENCE=-6.022e+23
float64 SMALL=+1.0E-12
string EMPTY=
string TOKEN=unquoted_token
string GREETING="hello world"
wstring WIDE_GREETING='γειά σου'
time ZERO_TIME=0
duration ONE_SECOND=1
# Inline comments terminate fields and defaults.
uint32 retry_count 3 # attempts before failure
float64 tolerance 0.001 # metres
string note "ready # not a comment" # actual comment
string[] roles [pilot, "flight lead"] # crew labels
@optional # recognized field annotation
string optional_note "telemetry enabled"
---
# Service-style response section.
bool accepted true
uint8 result_code 0
string result_message "prêt — 成功 🎉"
float64 score 99.75
fixture_msgs/Diagnostics diagnostics
---
# Action-style result and feedback section.
bool completed true
duration elapsed
string summary 'goal reached'
---
float32 progress 0.875
string phase tracking
geometry_msgs/Pose current_pose
==============================
MSG: std_msgs/Header
uint32 seq
time stamp
string frame_id
==============================
MSG: builtin_interfaces/Time
int32 sec
uint32 nanosec
==============================
MSG: geometry_msgs/Point
float64 x
float64 y
float64 z
==============================
MSG: geometry_msgs/Quaternion
float64 x
float64 y
float64 z
float64 w
==============================
MSG: geometry_msgs/Pose
geometry_msgs/Point position
geometry_msgs/Quaternion orientation
==============================
MSG: fixture_msgs/Nested
uint8 id 1
string label "nested fixture"
float32[] readings [1.0, 2.0, 3.0]
==============================
MSG: fixture_msgs/StringCatalog
string title "escape catalog"
string slash "C:\\ros\\logs"
string quote "say \"go\""
string control "bell\a form\f return\r vertical\v"
string octal "\141\142\143"
string hex "\x52\x4F\x53"
string unicode "\u03BB \U0001F6F0"
string unquoted escaped\ value
string[] mixed [plain, "double quoted", 'single quoted']
string[] unicode_items ["mañana", '雪', 🧪]
==============================
MSG: fixture_msgs/NumericCatalog
int32 plus_default +42
int32 minus_default -42
int32 grouped_default 1_234_567
float32 leading_dot .125
float32 trailing_dot 125.
float64 negative_exp -4.25e-3
float64 positive_exp +4.25E+3
float64[] edge_floats [.0, 0., +.5, -5.]
int64[] edge_integers [-1_000, +2_000, 3_000]
bool[] case_booleans [TRUE, false, True, FALSE]
==============================
MSG: fixture_msgs/ArrayCatalog
uint8[] empty_bytes []
int16[1] singleton [-1]
uint32[<=8] bounded [1, 2, 3]
string[] empty_strings []
string[2] pair [left, right]
wstring[<=2] wide_pair ['左', "右"]
fixture_msgs/Nested[] children
fixture_msgs/Nested[2] fixed_children
fixture_msgs/Nested[<=8] bounded_children
# Final Unicode comment includes an astral character: U+1F680 🚀
@optional
string final_status "done ✅"
