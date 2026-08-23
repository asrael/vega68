#ifndef V68_AFX_H
#define V68_AFX_H

#include "sys.h"

#define V68_AUDIO_CH(n)  ((volatile u8 *)(0xFF000400 + (n) * 0x40))
#define V68_AUDIO_KEYON  ((volatile u8 *)0xFF000800)
#define V68_AUDIO_LFO    ((volatile u8 *)0xFF000801)
#define V68_AUDIO_STATUS ((volatile u16 *)0xFF000802)
#define V68_AUDIO_ESEND  ((volatile u16 *)0xFF000810)
#define V68_AUDIO_EDELAY ((volatile u8 *)0xFF000812)
#define V68_AUDIO_EFB    ((volatile u8 *)0xFF000813)
#define V68_AUDIO_EVOL_L ((volatile u8 *)0xFF000814)
#define V68_AUDIO_EVOL_R ((volatile u8 *)0xFF000815)
#define V68_AUDIO_EFIR   ((volatile u8 *)0xFF000816)

#define V68_SOUND_POOL 2048

#define V68_PATCH_BRASS   0
#define V68_PATCH_STRINGS 1
#define V68_PATCH_CHOIR   2
#define V68_PATCH_EPIANO  3
#define V68_PATCH_BELL    4
#define V68_PATCH_FLUTE   5
#define V68_PATCH_HARP    6
#define V68_PATCH_ORGAN   7
#define V68_PATCH_BASS    8
#define V68_PATCH_LEAD    9
#define V68_PATCH_PLUCK   10
#define V68_PATCH_PERC    11

typedef struct {
	u8 op[4][7];
	u8 fb_alg, echo;
} V68Patch;

typedef struct {
	const char **bars;
	u8 bar_count;
	u8 patch;
	u8 ch;
	i8 transpose;
	u8 level;
	const V68Patch *patch_ptr;
} V68Track;

typedef struct {
	const V68Track *tracks;
	u8 track_count;
	u16 bar_frames;
} V68Section;

typedef struct {
	const V68Section *sections;
	u8 section_count, loop_section;
} V68Song;

extern const u16 v68_fnum[12];
extern const u16 v68_psg_period[12];
extern const V68Patch v68_patches[12];

void v68_fm_patch(u8 ch, const V68Patch *p);
i32  v68_song_start(const V68Song *song);
void v68_song_stop(void);

#endif
