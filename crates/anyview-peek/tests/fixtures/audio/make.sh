set -e
cd ~/av-wt/audio/fixtures
T=(-metadata title=SineSong -metadata artist=TheTones -metadata album=TestTones)
S=(-f lavfi -i sine=frequency=440:duration=2)
O=(-ar 22050 -ac 1)
ff() { ffmpeg -loglevel error -y "$@"; }
ff "${S[@]}" "${O[@]}" "${T[@]}" -b:a 32k plain.mp3
ff "${S[@]}" "${O[@]}" "${T[@]}" plain.flac
ff "${S[@]}" "${O[@]}" "${T[@]}" -b:a 32k plain.m4a
ff "${S[@]}" -i cover.jpg -map 0 -map 1 "${O[@]}" -c:a libmp3lame -b:a 32k -c:v copy -id3v2_version 3 "${T[@]}" -disposition:v attached_pic art.mp3
ff "${S[@]}" -i cover.png -map 0 -map 1 "${O[@]}" -c:a flac -c:v copy "${T[@]}" -disposition:v attached_pic art.flac
ff "${S[@]}" -i cover.jpg -map 0 -map 1 "${O[@]}" -c:a aac -b:a 32k -c:v copy "${T[@]}" -disposition:v attached_pic art.m4a
python3 -I - <<'P'
import struct,base64
png=open('cover.png','rb').read(); mime=b'image/png'
blk=struct.pack('>II',3,len(mime))+mime+struct.pack('>I',0)+struct.pack('>IIII',64,64,24,0)+struct.pack('>I',len(png))+png
open('/home/pohsuanlai/av-wt/audio/scratch.b64','w').write(base64.b64encode(blk).decode())
P
ff "${S[@]}" "${O[@]}" -c:a libvorbis -q:a 0 "${T[@]}" -metadata METADATA_BLOCK_PICTURE="$(cat ~/av-wt/audio/scratch.b64)" art.ogg
rm ~/av-wt/audio/scratch.b64
