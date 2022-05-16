#define TAU 6.283185307179586

#ifdef GL_ES
precision highp float;
precision highp sampler2D;
#endif

// static input
in vec2 basepoint;

// dynamic input
in vec2 iEndpointVector;
in vec2 iVelocityVector;
in vec4 iColor;
in float iLineWidth;
in float iLineOpacity;
in float iEndpointOpacity;

layout(std140) uniform Projection
{
  mat4 uProjection;
  mat4 uView;
};

layout(std140) uniform LineUniforms
{
  mediump float uLineWidth;
  mediump float uLineLength;
  mediump float uLineBeginOffset;
};

uniform float deltaTime;
uniform float uSpringVariance;
uniform mediump vec4 uColorWheel[6];

uniform float springNoiseOffset1;

uniform sampler2D velocityTexture;

// transform feedback output
out vec2 vEndpointVector;
out vec2 vVelocityVector;
out vec4 vColor;
out float vLineWidth;
out float vLineOpacity;
out float vEndpointOpacity;

float clampTo(float value, float max) {
  float current = value + 0.0001;
  return min(current, max) / current;
}

vec4 getColor(vec4 wheel[6], float angle) {
  float slice = TAU / 6.0;
  float rawIndex = mod(angle, TAU) / slice;
  float index = floor(rawIndex);
  float nextIndex = mod(index + 1.0, 6.0);
  float interpolate = fract(rawIndex);

  vec4 currentColor = wheel[int(index)];
  vec4 nextColor = wheel[int(nextIndex)];
  return mix(currentColor, nextColor, interpolate);
}

vec3 mod289(vec3 x) {
  return x - floor(x * (1.0 / 289.0)) * 289.0;
}

vec4 mod289(vec4 x) {
  return x - floor(x * (1.0 / 289.0)) * 289.0;
}

vec4 permute(vec4 x) {
  return mod289(((x * 34.0) + 1.0) * x);
}

vec4 taylorInvSqrt(vec4 r) {
  return 1.79284291400159 - 0.85373472095314 * r;
}

float snoise(vec3 v) {
  const vec2 C = vec2(1.0 / 6.0, 1.0 / 3.0);

  // First corner
  vec3 i = floor(v + dot(v, C.yyy));
  vec3 x0 = v - i + dot(i, C.xxx);

  // Other corners
  vec3 g = step(x0.yzx, x0.xyz);
  vec3 l = 1.0 - g;
  vec3 i1 = min(g.xyz, l.zxy);
  vec3 i2 = max(g.xyz, l.zxy);

  // x1 = x0 - i1  + 1.0 * C.xxx;
  // x2 = x0 - i2  + 2.0 * C.xxx;
  // x3 = x0 - 1.0 + 3.0 * C.xxx;
  vec3 x1 = x0 - i1 + C.xxx;
  vec3 x2 = x0 - i2 + C.yyy;
  vec3 x3 = x0 - 0.5;

  // Permutations
  i = mod289(i); // Avoid truncation effects in permutation
  vec4 p =
    permute(permute(permute(i.z + vec4(0.0, i1.z, i2.z, 1.0))
                          + i.y + vec4(0.0, i1.y, i2.y, 1.0))
                          + i.x + vec4(0.0, i1.x, i2.x, 1.0));

  // Gradients: 7x7 points over a square, mapped onto an octahedron.
  // The ring size 17*17 = 289 is close to a multiple of 49 (49*6 = 294)
  vec4 j = p - 49.0 * floor(p * (1.0 / 49.0)); // mod(p,7*7)

  vec4 x_ = floor(j * (1.0 / 7.0));
  vec4 y_ = floor(j - 7.0 * x_ ); // mod(j,N)

  vec4 x = x_ * (2.0 / 7.0) + 0.5 / 7.0 - 1.0;
  vec4 y = y_ * (2.0 / 7.0) + 0.5 / 7.0 - 1.0;

  vec4 h = 1.0 - abs(x) - abs(y);

  vec4 b0 = vec4(x.xy, y.xy);
  vec4 b1 = vec4(x.zw, y.zw);

  //vec4 s0 = vec4(lessThan(b0, 0.0)) * 2.0 - 1.0;
  //vec4 s1 = vec4(lessThan(b1, 0.0)) * 2.0 - 1.0;
  vec4 s0 = floor(b0) * 2.0 + 1.0;
  vec4 s1 = floor(b1) * 2.0 + 1.0;
  vec4 sh = -step(h, vec4(0.0));

  vec4 a0 = b0.xzyw + s0.xzyw * sh.xxyy;
  vec4 a1 = b1.xzyw + s1.xzyw * sh.zzww;

  vec3 g0 = vec3(a0.xy, h.x);
  vec3 g1 = vec3(a0.zw, h.y);
  vec3 g2 = vec3(a1.xy, h.z);
  vec3 g3 = vec3(a1.zw, h.w);

  // Normalise gradients
  vec4 norm = taylorInvSqrt(vec4(dot(g0, g0), dot(g1, g1), dot(g2, g2), dot(g3, g3)));
  g0 *= norm.x;
  g1 *= norm.y;
  g2 *= norm.z;
  g3 *= norm.w;

  // Mix final noise value
  vec4 m = max(0.6 - vec4(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), 0.0);
  m = m * m;
  m = m * m;

  vec4 px = vec4(dot(x0, g0), dot(x1, g1), dot(x2, g2), dot(x3, g3));
  return 42.0 * dot(m, px);
}

void main() {
  float springNoiseScale = 64.0;

  vec2 basepointInClipSpace = 0.5 + 0.5 * (uProjection * vec4(basepoint, 0.0, 1.0)).xy;
  vec2 velocity = texture(velocityTexture, basepointInClipSpace).xy;
  float noise = snoise(vec3(basepointInClipSpace * springNoiseScale, springNoiseOffset1));

  // Blend noise

  float variance = mix(1.0 - uSpringVariance, 1.0, 0.5 + 0.5 * noise);
  float inverseVariance = 1.0 - variance;
  float velocityDeltaVariance = mix(3.0, 25.0, inverseVariance);
  float momentumVariance = mix(3.0, 5.0, variance);

  float lineLength = 1.2; //0.6 // 1.4 // Do I need this? will I be changing this value?
  vVelocityVector = (1.0 - deltaTime * momentumVariance) * iVelocityVector + (lineLength * velocity - iEndpointVector) * velocityDeltaVariance * deltaTime;
  vEndpointVector = iEndpointVector + deltaTime * vVelocityVector;

  float widthBoost = clamp(2.5 * length(velocity), 0.0, 1.0);
  vLineWidth = widthBoost * widthBoost * (3.0 - widthBoost * 2.0);

  float angle = atan(velocity.x, velocity.y);
  vColor = getColor(uColorWheel, angle);
  vLineOpacity = widthBoost;

  // TODO: check this
  vEndpointOpacity = max(0.7, vLineOpacity);
}
