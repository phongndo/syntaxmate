# Basic ROS message fixture
# Unicode payload label: café 東京 🚀
std_msgs/Header header
bool enabled true
int32 sequence 42
float64 temperature 21.5
string frame_id "map"
string<=32 display_name "robot one"
wstring operator_name "Zoë"
uint8[4] rgba [12, 34, 56, 255]
float32[] samples [0.5, -1.25, 3.0]
geometry_msgs/Pose pose
builtin_interfaces/Time stamp
uint8 MODE_IDLE=0
uint8 MODE_ACTIVE=1
float64 DEFAULT_GAIN=1.0
bool DEFAULT_VISIBLE=true
string DEFAULT_TOPIC="/robots/探索"
# Astral character in a trailing comment: 🛰
duration timeout
