// Big Buck Bunny (c) Blender Foundation, CC BY 3.0: one 10 s clip as three progressive files, so a
// quality switch changes real resolution over the network.
export const PUBLIC_VIDEO = [360, 720, 1080].map(height => ({
  src: `https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/${height}/Big_Buck_Bunny_${height}_10s_1MB.mp4`,
  label: `${height}p`,
}));

// One adaptive HLS source: its renditions become the qualities. Apple's 16:9 test stream lists five,
// 1080p down to 234p; its advanced example's Dolby audio and captions fail HarmonyOS's player.
export const PUBLIC_STREAM = 'https://devstreaming-cdn.apple.com/videos/streaming/examples/bipbop_16x9/bipbop_16x9_variant.m3u8';
