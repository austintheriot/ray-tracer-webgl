#version 300 es

precision mediump float;

// PSEUDO-RANDOM NUMBER GENERATORS //////////////////////////////////////////////////////
// Hash functions by Nimitz:
// https://www.shadertoy.com/view/Xt3cDn

uint base_hash(uvec2 p) {
  p = 1103515245U * ((p >> 1U) ^ (p.yx));
  uint h32 = 1103515245U * ((p.x) ^ (p.y >> 3U));
  return h32 ^ (h32 >> 16);
}

vec2 hash2(inout float seed) {
  uint n = base_hash(floatBitsToUint(vec2(seed += .1, seed += .1)));
  uvec2 rz = uvec2(n, n * 48271U);
  return vec2(rz.xy & uvec2(0x7fffffffU)) / float(0x7fffffff);
}

vec3 hash3(inout float seed) {
  uint n = base_hash(floatBitsToUint(vec2(seed += .1, seed += .1)));
  uvec3 rz = uvec3(n, n * 16807U, n * 48271U);
  return vec3(rz & uvec3(0x7fffffffU)) / float(0x7fffffff);
}

// INPUTS / OUTPUTS //////////////////////////////////////////////////////
in vec2 v_position;

out vec4 o_color;

// video frame, received as a 2d texture
uniform sampler2D u_texture;
uniform float u_width;
uniform float u_height;
uniform float u_time;
uniform int u_samples_per_pixel;
uniform float u_aspect_ratio;
uniform float u_viewport_height;
uniform float u_viewport_width;
uniform float u_focal_length;
uniform vec3 u_camera_origin;
uniform vec3 u_horizontal;
uniform vec3 u_vertical;
uniform vec3 u_lower_left_corner;
uniform int u_max_depth;
uniform int u_render_count;
uniform bool u_should_average;
uniform float u_last_frame_weight;

// STRUCTS //////////////////////////////////////////////////////
struct Ray {
  vec3 origin;
  vec3 direction;
};

struct Material {
// 0 = Diffuse
// 1 = Metal
// 2 = glass
  int type;
  vec3 albedo; // or "reflectance"
  float fuzz; // used for duller metals
  float refraction_index; // used for glass
};

struct Sphere {
  vec3 center;
  float radius;
  Material material;
};

struct HitRecord {
  vec3 hit_point;
  float hit_t;
  vec3 normal;
  bool front_face;
  Material material;
};

// GLOBALS ////////////////////////////////////////////////////////
const float PI = 3.141592653589793;
const float MAX_T = 1e5;
const float MIN_T = 0.001;
float global_seed = 0.;

// FUNCTIONS //////////////////////////////////////////////////////
vec3 ray_at(in Ray r, float hit_t) {
  return r.origin + r.direction * hit_t;
}

float length_squared(in vec3 v) {
  return pow(length(v), 2.);
}

vec3 random_in_unit_sphere() {
  // no idea how this algorithm works, but it works much better than the one I was using.
  // From reinder https://www.shadertoy.com/view/llVcDz
  vec3 h = hash3(global_seed) * vec3(2., PI * 2., 1.) - vec3(1., 0., 0.);
  float phi = h.y;
  float r = pow(h.z, 1. / 3.);
  return r * vec3(sqrt(1. - h.x * h.x) * vec2(sin(phi), cos(phi)), h.x);
}

vec3 random_unit_vec() {
  return normalize(random_in_unit_sphere());
}

// records whether a hit happened to the front or back face of an object
void set_hit_record_front_face(inout HitRecord hit_record, in Ray r, in vec3 outward_normal) {
  hit_record.front_face = dot(r.direction, outward_normal) < 0.;
  if (hit_record.front_face) {
    hit_record.normal = outward_normal;
  } else {
    hit_record.normal = -outward_normal;
  }
}

vec3 refract(in vec3 uv, in vec3 normal, float eta_i_over_eta_t) {
  float cos_theta = min(dot(-uv, normal), 1.);
  vec3 r_out_perp = eta_i_over_eta_t * (uv + cos_theta * normal);
  vec3 r_out_parallel = -sqrt(abs(1.0 - length_squared(r_out_perp))) * normal;
  return r_out_perp + r_out_parallel;
}

bool hit_sphere(in Sphere sphere, in Ray r, in float t_min, in float t_max, inout HitRecord hit_record) {
  vec3 oc = r.origin - sphere.center;
  float a = length_squared(r.direction);
  float half_b = dot(oc, r.direction);
  float c = length_squared(oc) - pow(sphere.radius, 2.);
  float discriminant = pow(half_b, 2.) - a * c;

  // no hit
  if (discriminant < 0.)
    return false;

// there was a hit, but it's not within an acceptable range
  float sqrtd = sqrt(discriminant);
  float root = (-half_b - sqrtd) / a;
  if (root < t_min || t_max < root) {
    root = (-half_b + sqrtd) / a;
    if (root < t_min || t_max < root) {
      return false;
    }
  }

  hit_record.material = sphere.material;
  hit_record.hit_t = root;
  hit_record.hit_point = ray_at(r, hit_record.hit_t);
  vec3 outward_normal = (hit_record.hit_point - sphere.center) / sphere.radius;
  set_hit_record_front_face(hit_record, r, outward_normal);
  return true;
}

bool hit_world(in Ray r, in float t_min, in float t_max, inout HitRecord hit_record) {
  // compose the scene (hardcoded for now)
  // diffuse ground
  Sphere ground = Sphere(vec3(0., -100.5, -1.), 100., Material(0, vec3(0.75, 0.6, 0.5), 0., 0.));
  // diffuse blue
  Sphere center = Sphere(vec3(0., 0., -1.), 0.5, Material(0, vec3(0.3, 0.3, 0.4), 0., 0.));
  // solid glass
  Sphere left = Sphere(vec3(-1., 0., 0.), 0.5, Material(2, vec3(1., 1., 1.), 0., 1.5));
  // hollow glass
  Sphere behind = Sphere(vec3(0., 0., 1.), -0.5, Material(2, vec3(1., 1., 1.), 0., 1.5));
  // small shiny metal
  Sphere left_small = Sphere(vec3(-0.45, -0.4, -0.7), 0.1, Material(1, vec3(1., 1., 1.), 0., 0.));
  // dull metal
  Sphere right = Sphere(vec3(1., 0., 0.), 0.5, Material(1, vec3(1., 1., 1.), 0.5, 0.));
  Sphere sphere_list[] = Sphere[6] (center, left, behind, left_small, right, ground);

  // test whether any geometry was hit. If it was, the hit_record will be updated with
  // the new hit data if the new hit was closer to the camera than the previous hit
  bool hit_anything = false;
  float closest_so_far = t_max;
  HitRecord temp_hit_record;

  for(int i = 0; i < sphere_list.length(); i++) {
    Sphere sphere = sphere_list[i];
    if (hit_sphere(sphere, r, t_min, closest_so_far, temp_hit_record)) {
      hit_anything = true;
      closest_so_far = temp_hit_record.hit_t;
      hit_record = temp_hit_record;
    }
  }

  return hit_anything;
}

bool near_zero(in vec3 v) {
  float low_extreme = 1e-6;
  return (v.x < low_extreme) && (v.y < low_extreme) && (v.z < low_extreme);
}

// returns the new direction ray after a reflection
vec3 reflect(in vec3 v, in vec3 normal) {
  return v - 2. * dot(v, normal) * normal;
}

// Schlick's approximation for reflectance
float reflectance(in float cosine, in float reflection_index) {
  float r0 = pow((1. - reflection_index) / (1. + reflection_index), 2.);
  return r0 + (1. - r0) * pow((1. - cosine), 5.);
}

// scatters a ray depending on what material was intersected with
bool scatter(in Ray r, in HitRecord hit_record, out vec3 attenuation, out Ray scattered_ray) {
  // 0 = diffuse material (lambertian reflection)
  if (hit_record.material.type == 0) {
    // color attenuation on reflection
    attenuation = hit_record.material.albedo;

    // shoot ray off in random direction again
    vec3 scatter_direction = hit_record.normal + random_unit_vec();

    // scatter direction can become close to 0 if opposite the normal vector 
    // (which can cause infinities later on)
    if (near_zero(scatter_direction)) {
      scatter_direction = hit_record.normal;
    }
    scattered_ray = Ray(hit_record.hit_point, scatter_direction);

    return true;
  } 

  // 1 = metal
  if (hit_record.material.type == 1) {
    // color attenuation on reflection
    attenuation = hit_record.material.albedo;

    // reflect ray off the surface
    vec3 reflected_direction = reflect(r.direction, hit_record.normal);

    // add in "fuzz" (optional)
    vec3 fuzzed_direction = reflected_direction + hit_record.material.fuzz * random_in_unit_sphere();
    scattered_ray = Ray(hit_record.hit_point, fuzzed_direction);

    // count any rays that are reflected below the surface as  "absorbed"
    bool reflected_above_surface = dot(hit_record.normal, fuzzed_direction) > 0.;

    return reflected_above_surface;
  } 

  // 2 = glass
  if (hit_record.material.type == 2) {
    // color attenuation on reflection
    attenuation = hit_record.material.albedo;

    // refraction differs when colliding from the front or back face
    float refraction_ratio = hit_record.front_face ? (1.0 / hit_record.material.refraction_index) : hit_record.material.refraction_index;

    vec3 unit_direction = normalize(r.direction);
    float cos_theta = min(dot(-unit_direction, hit_record.normal), 1.0);
    float sin_theta = sqrt(1.0 - cos_theta * cos_theta);

    // cannot refract when there is no real solution to Snell's law
    bool cannot_refract = refraction_ratio * sin_theta > 1.0;

    // there is a random chance of the ray reflecting
    // --chance increases as reflectance approximation increases
    float reflectance_amount = reflectance(cos_theta, refraction_ratio);
    float random_float = hash2(global_seed).x;

    // when the ray cannot refract (or when it's reflectance 
    // approximation is high), it reflects instead
    vec3 direction;
    if (cannot_refract || reflectance_amount > random_float) {
      direction = reflect(unit_direction, hit_record.normal);
    } else {
      direction = refract(unit_direction, hit_record.normal, refraction_ratio);
    }

    scattered_ray = Ray(hit_record.hit_point, direction);

    // never absorbs light
    return true;
  } 

// unrecognized material integer (likely an error)
  return false;
}

// determine the color that a ray should be
vec3 ray_color(in Ray r) {
  vec3 color = vec3(1.);

  for(int i = 0; i < u_max_depth; i++) {
    // test for collisions with any geometry
    // hit record gets modified with hit details if there was a hit
    HitRecord hit_record;
    if (hit_world(r, MIN_T, MAX_T, hit_record)) {
      vec3 attenuation;
      Ray scattered_ray;
      bool did_scatter = scatter(r, hit_record, attenuation, scattered_ray);

      if (did_scatter) {
        r = scattered_ray;
        color *= attenuation;
      } else {
        return vec3(0.);
      }

    } else {
        // no hit, return the sky gradient background
      vec3 unit_direction = normalize(r.direction);
      float t = 0.5 * (unit_direction.y + 1.0);
      vec3 gradient = mix(vec3(1.0, 1.0, 1.0), vec3(0.5, 0.7, 1.0), t);
      return color * gradient;
    }
  }

  return color;
}

void main() {
  // this seed initialization is by reinder https://www.shadertoy.com/view/llVcDz
  global_seed = float(base_hash(floatBitsToUint(v_position))) / float(0xffffffffU) + u_time;

  // get current position on viewport, mapped from -1->1 to 0->1
  // (i.e. percentage of width, percentage of height)
  vec2 uv = (v_position + 1.) * 0.5;

  // accumulate color per pixel
  vec3 color = vec3(0.);
  for(int i = 0; i < u_samples_per_pixel; i++) {
    vec2 random = hash2(global_seed);
    vec2 random_from_0_to_1 = (random * 0.5) + 1.0;
    vec2 random_within_pixel = random_from_0_to_1 / vec2(u_width, u_height);

    // uv +/- the value of 1 pixel
    vec2 randomized_uv = uv + random_within_pixel;

    // create ray from camera origin to viewport
    vec3 ray_direction = u_lower_left_corner + randomized_uv.x * u_horizontal + randomized_uv.y * u_vertical - u_camera_origin;
    Ray r = Ray(u_camera_origin, ray_direction);

    color += ray_color(r);
  }

  // scale color by number of samples
  float scale = (1. / float(u_samples_per_pixel));
  color *= scale;

  // gamma correction
  color = sqrt(color);

  vec4 prev_frame = texture(u_texture, uv);
  float render_count = float(u_render_count);

  if (u_should_average) {
    // average this frame with previous frames
    if (prev_frame.a == 0. || u_render_count == 0 || u_render_count == 1) {
      o_color = vec4(color, 1.);
    } else {
      float total_frames = render_count + u_last_frame_weight;
      vec3 merged_color = (prev_frame.rgb * render_count + color * u_last_frame_weight) / total_frames;
      o_color = vec4(merged_color, 1.);
    }
  } else {
    // plain rendering
    o_color = vec4(color, 1.);
  }
}