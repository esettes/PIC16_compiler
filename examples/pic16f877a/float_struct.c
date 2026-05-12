struct Sensor {
    float raw;
    float gain;
};

struct Sensor sensor;
float result;

void main(void) {
    sensor.raw = 1.5f;
    sensor.gain = 2.0f;
    result = sensor.raw * 2.0f;
}
