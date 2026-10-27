// 后期：半分辨率景深（按深度算弥散圈、黄金角螺旋采样）→ 发光层叠加 → 物理式泛光 → 胶片色调、暗角、色散、颗粒
import * as THREE from 'three';
import { GLSL_NOISE } from './lib.js';

const VS = /* glsl */ `varying vec2 vUv; void main(){ vUv = uv; gl_Position = vec4(position.xy, 0., 1.); }`;

export class Post {
  constructor(renderer, W, H) {
    this.r = renderer; this.W = W; this.H = H;
    const hf = { type: THREE.HalfFloatType, minFilter: THREE.LinearFilter, magFilter: THREE.LinearFilter, depthBuffer: false };
    this.depth = new THREE.DepthTexture(W, H, THREE.FloatType);
    this.scene = new THREE.WebGLRenderTarget(W, H, { ...hf, depthBuffer: true, depthTexture: this.depth });
    this.half = new THREE.WebGLRenderTarget(W >> 1, H >> 1, hf);
    this.blur = new THREE.WebGLRenderTarget(W >> 1, H >> 1, hf);
    // 发光物体（光丝、粒子）直接加到 comp 上；遮挡由它们的着色器读 scene 的深度自己判断
    this.comp = new THREE.WebGLRenderTarget(W, H, hf);
    this.blooms = [];
    for (let i = 1, w = W >> 1, h = H >> 1; i <= 6; i++, w >>= 1, h >>= 1) this.blooms.push(new THREE.WebGLRenderTarget(w, h, hf));
    this.ups = this.blooms.slice(0, -1).map((b) => new THREE.WebGLRenderTarget(b.width, b.height, hf));

    this.quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2));
    this.qs = new THREE.Scene(); this.qs.add(this.quad); this.qc = new THREE.Camera();
    this.quad.frustumCulled = false;

    const depthFns = /* glsl */ `
      uniform sampler2D tDepth; uniform float near, far, focus, aperture, maxCoc;
      float viewZ(vec2 uv){ float d = texture2D(tDepth, uv).x; return near * far / (far - d * (far - near)); }
      float coc(vec2 uv){ float z = viewZ(uv); return clamp(aperture * (z - focus) / z, -maxCoc, maxCoc); }`;

    this.mDown = this.mat(/* glsl */ `
      uniform sampler2D tColor; uniform vec2 px; varying vec2 vUv; ${depthFns}
      void main(){
        vec3 c = texture2D(tColor, vUv + px * vec2(-.5,-.5)).rgb + texture2D(tColor, vUv + px * vec2(.5,-.5)).rgb
               + texture2D(tColor, vUv + px * vec2(-.5,.5)).rgb + texture2D(tColor, vUv + px * vec2(.5,.5)).rgb;
        float k = coc(vUv);
        k = abs(k) < abs(coc(vUv + px * .5)) ? coc(vUv + px * .5) : k;
        gl_FragColor = vec4(c * .25, k * .5);
      }`, { tColor: null, px: new THREE.Vector2(), tDepth: this.depth, near: 0.1, far: 100, focus: 10, aperture: 0, maxCoc: 24 });

    // 28 个黄金角螺旋采样，单位是半分辨率像素
    this.mBlur = this.mat(/* glsl */ `
      uniform sampler2D tHalf; uniform vec2 px; uniform float maxCoc; varying vec2 vUv;
      void main(){
        vec4 c0 = texture2D(tHalf, vUv); float r0 = abs(c0.a);
        vec3 acc = c0.rgb; float wsum = 1.; float fg = max(0., -c0.a);
        float R = maxCoc * .5;
        for (int i = 1; i < 28; i++) {
          float fi = float(i);
          float rr = sqrt(fi / 28.) * R; float a = fi * 2.39996323;
          vec2 o = vec2(cos(a), sin(a)) * rr;
          vec4 s = texture2D(tHalf, vUv + o * px);
          float rs = abs(s.a);
          // 采样点自己的弥散圈够大才会洒到这里；前景（负值）可以盖住清晰的背景
          float w = smoothstep(rr - 1.5, rr + .5, rs);
          if (s.a > 0. && c0.a > 0.) w *= smoothstep(rr - 1.5, rr + .5, r0 + 2.);
          acc += s.rgb * w; wsum += w;
          fg = max(fg, max(0., -s.a) * smoothstep(rr - 1.5, rr + .5, -s.a));
        }
        gl_FragColor = vec4(acc / wsum, fg);
      }`, { tHalf: null, px: new THREE.Vector2(), maxCoc: 24 });

    this.mComp = this.mat(/* glsl */ `
      uniform sampler2D tColor, tBlur; varying vec2 vUv; ${depthFns}
      void main(){
        vec3 sharp = texture2D(tColor, vUv).rgb;
        vec4 b = texture2D(tBlur, vUv);
        float k = abs(coc(vUv));
        float m = max(smoothstep(.6, 2.5, k), smoothstep(.6, 2.5, b.a * 2.));
        gl_FragColor = vec4(mix(sharp, b.rgb, m), 1.);
      }`, { tColor: null, tBlur: null, tDepth: this.depth, near: 0.1, far: 100, focus: 10, aperture: 0, maxCoc: 24 });

    this.mBloomDown = this.mat(/* glsl */ `
      uniform sampler2D tSrc; uniform vec2 px; varying vec2 vUv;
      void main(){
        vec3 c = texture2D(tSrc, vUv).rgb * .25;
        c += (texture2D(tSrc, vUv + px * vec2(-1,-1)).rgb + texture2D(tSrc, vUv + px * vec2(1,-1)).rgb
            + texture2D(tSrc, vUv + px * vec2(-1,1)).rgb + texture2D(tSrc, vUv + px * vec2(1,1)).rgb) * .125;
        c += (texture2D(tSrc, vUv + px * vec2(-2,0)).rgb + texture2D(tSrc, vUv + px * vec2(2,0)).rgb
            + texture2D(tSrc, vUv + px * vec2(0,-2)).rgb + texture2D(tSrc, vUv + px * vec2(0,2)).rgb) * .0625;
        gl_FragColor = vec4(c, 1.);
      }`, { tSrc: null, px: new THREE.Vector2() });

    this.mBloomUp = this.mat(/* glsl */ `
      uniform sampler2D tSrc, tBase; uniform vec2 px; varying vec2 vUv;
      void main(){
        vec3 c = texture2D(tSrc, vUv).rgb * 4.;
        c += (texture2D(tSrc, vUv + px * vec2(-1,0)).rgb + texture2D(tSrc, vUv + px * vec2(1,0)).rgb
            + texture2D(tSrc, vUv + px * vec2(0,-1)).rgb + texture2D(tSrc, vUv + px * vec2(0,1)).rgb) * 2.;
        c += (texture2D(tSrc, vUv + px * vec2(-1,-1)).rgb + texture2D(tSrc, vUv + px * vec2(1,-1)).rgb
            + texture2D(tSrc, vUv + px * vec2(-1,1)).rgb + texture2D(tSrc, vUv + px * vec2(1,1)).rgb);
        gl_FragColor = vec4(c / 16. + texture2D(tBase, vUv).rgb, 1.);
      }`, { tSrc: null, tBase: null, px: new THREE.Vector2() });

    this.mFinal = this.mat(/* glsl */ `
      uniform sampler2D tComp, tBloom; uniform float bloom, exposure, fade, grain, frame, vignette, ca, lift;
      uniform vec3 tint; varying vec2 vUv;
      ${GLSL_NOISE}
      // AgX 风格的胶片曲线：高光柔和地压下去，不会一片死白
      vec3 filmic(vec3 x){ x = max(x, 0.); vec3 a = x * (x * 2.51 + .03); vec3 b = x * (x * 2.43 + .59) + .14; return clamp(a / b, 0., 1.); }
      void main(){
        vec2 d = vUv - .5;
        float r2 = dot(d, d);
        vec3 c;
        c.r = texture2D(tComp, vUv - d * ca * r2).r;
        c.g = texture2D(tComp, vUv).g;
        c.b = texture2D(tComp, vUv + d * ca * r2).b;
        c += texture2D(tBloom, vUv).rgb * bloom;
        c *= exposure * tint;
        c = filmic(c);
        c = pow(c, vec3(1. / 2.2));
        c *= 1. - vignette * smoothstep(.15, .85, r2 * 2.2);
        c = mix(vec3(lift), vec3(1.), c);
        float g = hash12(vUv * vec2(1920., 1080.) + frame * 17.31) + hash12(vUv * vec2(1931., 1077.) - frame * 7.7) - 1.;
        float lum = dot(c, vec3(.299, .587, .114));
        c += g * grain * (1. - lum * .6);
        c *= fade;
        c += (hash12(vUv * 1000. + frame) - .5) / 255.;
        gl_FragColor = vec4(c, 1.);
      }`, { tComp: null, tBloom: null, bloom: 0.08, exposure: 1, fade: 1, grain: 0.035, frame: 0, vignette: 0.35, ca: 0.012, lift: 0.0, tint: new THREE.Vector3(1, 1, 1) });
  }

  mat(fs, uniforms) {
    const u = {}; for (const [k, v] of Object.entries(uniforms)) u[k] = { value: v };
    return new THREE.ShaderMaterial({ vertexShader: VS, fragmentShader: fs, uniforms: u, depthTest: false, depthWrite: false });
  }
  pass(m, target) { this.quad.material = m; this.r.setRenderTarget(target); this.r.render(this.qs, this.qc); }

  // main: 不透明场景；glow: 发光层（加法混合，深度测试用 main 的深度）；cam: 镜头参数
  render(main, glow, camera, lens, look) {
    const r = this.r;
    r.setRenderTarget(this.scene); r.setClearColor(this.clear ?? 0x000000, 1); r.clear(); r.render(main, camera);

    const dof = lens.aperture > 0.01;
    const setDepth = (m) => { m.uniforms.near.value = camera.near; m.uniforms.far.value = camera.far; m.uniforms.focus.value = lens.focus; m.uniforms.aperture.value = lens.aperture; m.uniforms.maxCoc.value = lens.maxCoc ?? 24; };
    if (dof) {
      setDepth(this.mDown);
      this.mDown.uniforms.tColor.value = this.scene.texture; this.mDown.uniforms.px.value.set(1 / this.W, 1 / this.H);
      this.pass(this.mDown, this.half);
      this.mBlur.uniforms.tHalf.value = this.half.texture; this.mBlur.uniforms.px.value.set(2 / this.W, 2 / this.H);
      this.mBlur.uniforms.maxCoc.value = lens.maxCoc ?? 24;
      this.pass(this.mBlur, this.blur);
    }
    setDepth(this.mComp);
    if (!dof) this.mComp.uniforms.aperture.value = 0;
    this.mComp.uniforms.tColor.value = this.scene.texture; this.mComp.uniforms.tBlur.value = dof ? this.blur.texture : this.scene.texture;
    this.pass(this.mComp, this.comp);

    if (glow) { r.setRenderTarget(this.comp); r.autoClear = false; r.render(glow, camera); r.autoClear = true; }

    // 泛光金字塔
    let src = this.comp;
    for (const b of this.blooms) {
      this.mBloomDown.uniforms.tSrc.value = src.texture; this.mBloomDown.uniforms.px.value.set(1 / src.width, 1 / src.height);
      this.pass(this.mBloomDown, b); src = b;
    }
    for (let i = this.ups.length - 1; i >= 0; i--) {
      this.mBloomUp.uniforms.tSrc.value = src.texture; this.mBloomUp.uniforms.tBase.value = this.blooms[i].texture;
      this.mBloomUp.uniforms.px.value.set(1 / src.width, 1 / src.height);
      this.pass(this.mBloomUp, this.ups[i]); src = this.ups[i];
    }
    const f = this.mFinal.uniforms;
    f.tComp.value = this.comp.texture; f.tBloom.value = src.texture;
    for (const [k, v] of Object.entries(look)) { if (k === 'tint') f.tint.value.set(...v); else if (f[k]) f[k].value = v; }
    this.pass(this.mFinal, null);
  }
}
