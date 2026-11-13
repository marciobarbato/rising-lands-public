/* Shadow of the original initialized sound queue for app playback only.
 *
 * Line-by-line port of Audio's validated reference
 * pesquisas-paralelas/audio/fila_audio.py (AudioQueue: insert/post, set_volume,
 * stop_handle, compact, destroy_waiting, transfer_to_playing, destroy_only_sound,
 * logical/physical slot tables, clear_list, the entry-word-256 dictator and
 * manage) and bancos_som.carregar_info_som (FindIdInBanque/GetDataHeader
 * variant cursor). It runs over a private image at the original addresses:
 * globals 0x498f14 (88 bytes), BSS 0x521358 (3804 bytes), the HED bodies the
 * bank records point to, and a sparse map of caller handle words. The engine's
 * game memory is never read or written: the original game state is the
 * unopened-library domain the engine reproduces.
 *
 * DirectSound is replaced by abstract buffer ids. Their status words follow
 * DSB_IsPlaying (bit 1) / DSB_IsStopped ((status&5)==0): Play sets 1 (plus 4
 * when looping), Stop and the device's end-of-data answer clear it. Out-of-
 * domain cases that make the Python reference raise (record index or slot
 * maximum outside the captured range, volume lookup outside 0..16) are counted
 * in domain_faults and continue with the original's arithmetic, never by
 * touching game state. */
#include "MacOSAudioMapper.h"
#include <stdlib.h>
#include <string.h>

#define GLB 0x498f14u
#define GLB_N 88u
#define BSS 0x521358u
#define BSS_N 3804u
#define HDR 0x10000000u
#define HDR_STRIDE 0x10000u
#define DICT 0x0f000000u
#define DEVICE_POINTER 0x0e000000u
#define TABLE 0x52137cu
#define STRIDE 44u
#define INDEX 0x498f68u
#define GAMEVOL 0x498f58u
#define DIRTY 0x498f5cu
#define PSBMAX 0x498f60u
#define LSBMAX 0x498f64u
#define PSB 0x52211cu
#define LSB 0x52216cu
#define DEVICE 0x522228u
#define VARIANT 0x5221bcu
#define REQUEST 0x521f84u
#define VOLDIRTY 0x521f88u
#define DFLAG 0x521f8cu
#define DACTIVE 0x521f90u
#define DREC 0x521f94u
#define DSLOT 0x521fb4u
#define DREF 0x521fbcu
#define DSLOT2 0x521fc0u
#define ALLOCS 0x522230u
#define NONE 0xffffffffu
#define REFS 4096u
#define BUFFERS 64u
#define COMMANDS 2048u

static const uint32_t BANKS[3]={0x521fc8u,0x521fe4u,0x522000u};
static const uint32_t SELECTORS[3]={0x41u,0x42u,0x50u};
/* Initialized original bytes 0x498f14..0x498f6c (Audio M3 capture
 * m3-pedidos-original-v1.json, region sha256 0b2870b9...5706): the 17-word
 * DirectSound attenuation table, game volume 256, dirty 1, PSB/LSB maxima -1
 * and waiting maximum 19. */
static const int32_t ORIGINAL_GLOBALS[22]={-10000,-4000,-3500,-3000,-2500,-2000,-1750,-1500,-1250,-1000,
 -750,-500,-250,-180,-125,-62,0,256,1,-1,-1,19};

typedef struct {uint32_t key,value;} Ref;
typedef struct {uint32_t id,status;} Buffer;
struct MacOSAudioMapper {
 uint8_t glb[GLB_N],bss[BSS_N],dict[8],hdr[3][HDR_STRIDE];uint32_t hdr_len[3];
 Ref refs[REFS];Buffer buffers[BUFFERS];uint32_t next_buffer;
 MacOSAudioCommand commands[COMMANDS];uint32_t command_count,sequence,frame,phase;
 MacOSAudioStats stats;
};
typedef MacOSAudioMapper M;

static uint32_t le(const uint8_t*p){return p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);}
static int32_t i32(uint32_t v){return (int32_t)v;}
static uint8_t *at(M*m,uint32_t a,uint32_t n){
 if(a>=GLB&&a-GLB<=GLB_N-n)return m->glb+(a-GLB);
 if(a>=BSS&&a-BSS<=BSS_N-n)return m->bss+(a-BSS);
 for(unsigned b=0;b<3;b++){uint32_t base=HDR+b*HDR_STRIDE;if(a>=base&&a-base<HDR_STRIDE&&m->hdr_len[b]>=n&&a-base<=m->hdr_len[b]-n)return m->hdr[b]+(a-base);}
 if(a>=DICT&&a-DICT<=8-n)return m->dict+(a-DICT);
 return NULL;
}
/* Caller handle words (unit +0x104, building +0x100, voice channels...) live in
 * game memory, which always holds -1 there in the engine's domain. */
static Ref *ref(M*m,uint32_t a,int create){
 uint32_t h=(a*2654435761u)>>20;
 for(uint32_t i=0;i<REFS;i++){Ref*r=&m->refs[(h+i)&(REFS-1)];if(r->key==a)return r;if(!r->key){if(!create)return NULL;r->key=a;r->value=NONE;return r;}}
 if(create)m->stats.handle_overflow++;
 return NULL;
}
static uint32_t rd(M*m,uint32_t a){uint8_t*p=at(m,a,4);if(p)return le(p);if(!a)return NONE;Ref*r=ref(m,a,0);return r?r->value:NONE;}
static void wr(M*m,uint32_t a,uint32_t v){uint8_t*p=at(m,a,4);if(p){p[0]=(uint8_t)v;p[1]=(uint8_t)(v>>8);p[2]=(uint8_t)(v>>16);p[3]=(uint8_t)(v>>24);return;}if(!a){m->stats.domain_faults++;return;}Ref*r=ref(m,a,1);if(r)r->value=v;}
static uint32_t rb(M*m,uint32_t a){uint8_t*p=at(m,a,1);if(!p){m->stats.domain_faults++;return 0;}return *p;}
static void wb(M*m,uint32_t a,uint32_t v){uint8_t*p=at(m,a,1);if(!p){m->stats.domain_faults++;return;}*p=(uint8_t)v;}
static uint32_t rw(M*m,uint32_t a){uint8_t*p=at(m,a,2);if(!p){m->stats.domain_faults++;return 0;}return p[0]|((uint32_t)p[1]<<8);}
static void copy44(M*m,uint32_t to,uint32_t from){uint8_t*t=at(m,to,44),*f=at(m,from,44);if(!t||!f){m->stats.domain_faults++;return;}memmove(t,f,44);}

static void emit(M*m,uint32_t op,uint32_t buffer,uint32_t source,int32_t volume,uint32_t loop){
 m->stats.commands++;if(op==MACOS_AUDIO_PLAY)m->stats.plays++;
 if(m->command_count==COMMANDS){m->stats.dropped_commands++;return;}
 m->commands[m->command_count++]=(MacOSAudioCommand){op,buffer,source,volume,loop,m->frame,m->phase,m->sequence++};
}
static Buffer *buffer(M*m,uint32_t id){if(!id)return NULL;for(unsigned i=0;i<BUFFERS;i++)if(m->buffers[i].id==id)return &m->buffers[i];return NULL;}
static uint32_t new_buffer(M*m){
 for(unsigned i=0;i<BUFFERS;i++)if(!m->buffers[i].id){uint32_t id=m->next_buffer++;if(!m->next_buffer)m->next_buffer=1;m->buffers[i]=(Buffer){id,0};return id;}
 m->stats.buffer_overflow++;return 0;
}
/* GetStatus 0x48be90/0x48beb9: null guards answer playing 0 / stopped 1. */
static int playing(M*m,uint32_t p){if(!p)return 0;Buffer*b=buffer(m,p);return b&&(b->status&1);}
static int stopped(M*m,uint32_t p){if(!p)return 1;Buffer*b=buffer(m,p);return !b||(b->status&5)==0;}
static int stop_buffer(M*m,uint32_t p){if(stopped(m,p))return 1;emit(m,MACOS_AUDIO_STOP,p,0,0,0);buffer(m,p)->status=0;return 1;}

static uint32_t maximum(M*m){uint32_t v=rd(m,INDEX);if(v<19||v>70){m->stats.domain_faults++;return v<19?19:70;}return v;}
static uint32_t address(M*m,uint32_t index){if(index>70){m->stats.domain_faults++;index=70;}return TABLE+index*STRIDE;}
static uint32_t reserve(M*m){uint32_t v=maximum(m);if(v==70)return NONE;wr(m,INDEX,v+1);return v+1;}
static void give_back(M*m){uint32_t v=maximum(m);if(v!=19)wr(m,INDEX,v-1);}
static int compact(M*m,uint32_t a,int clear){
 uint32_t reference=rd(m,a+40),saved=rd(m,reference);
 if(maximum(m)==19)return 0;
 if(clear)wr(m,reference,NONE);
 uint32_t last=address(m,maximum(m));
 if(last!=a){copy44(m,a,last);wr(m,rd(m,a+40),saved);}
 wb(m,address(m,maximum(m))+8,0);wr(m,INDEX,maximum(m)-1);return 1;
}
static int destroy_waiting(M*m,uint32_t index){
 uint32_t r=address(m,index),top=maximum(m);
 if(index==top){wr(m,INDEX,top-1);wb(m,r+8,0);return 0;}
 uint32_t saved=rd(m,rd(m,r+40));
 if(top==19)return 1;
 uint32_t last=address(m,top);
 if(last!=r){copy44(m,r,last);wr(m,rd(m,r+40),saved);}
 wr(m,INDEX,maximum(m)-1);wb(m,last+8,0);return 1;
}
static void destroy_buffer(M*m,uint32_t slot){
 uint32_t p=rd(m,slot);
 if(p){emit(m,MACOS_AUDIO_RELEASE,p,0,0,0);Buffer*b=buffer(m,p);if(b)*b=(Buffer){0,0};wr(m,slot,0);}
}
static int32_t slot_max(M*m,uint32_t counter){int32_t v=i32(rd(m,counter));if(v<-1||v>9){m->stats.domain_faults++;return v<-1?-1:9;}return v;}
static void destroy_logical(M*m,uint32_t slot){
 if(playing(m,rd(m,slot)))stop_buffer(m,rd(m,slot));
 destroy_buffer(m,slot);wr(m,slot+4,0);
 int32_t top=i32(rd(m,LSBMAX));
 if(top<0)return;
 if(top>9){m->stats.domain_faults++;top=9;}
 uint32_t last=LSB+(uint32_t)top*8;
 if(slot!=last){wr(m,slot,rd(m,last));wr(m,slot+4,rd(m,last+4));wr(m,rd(m,slot+4)+32,slot);wr(m,last,0);wr(m,last+4,0);}
 wr(m,LSBMAX,(uint32_t)(top-1));
}
static int replace_slot(M*m,uint32_t slot,int logical){
 uint32_t base=logical?LSB:PSB,counter=logical?LSBMAX:PSBMAX;int32_t top=slot_max(m,counter);
 if(top<0)return 0;
 if(slot<base||slot>base+72||(slot-base)%8){m->stats.domain_faults++;return 0;}
 uint32_t last=base+(uint32_t)top*8;
 if(slot!=last){wr(m,slot,rd(m,last));wr(m,slot+4,rd(m,last+4));wr(m,rd(m,slot+4)+32,slot);wr(m,last,0);wr(m,last+4,0);}
 wr(m,counter,(uint32_t)(top-1));return 1;
}
static uint32_t find_loaded(M*m,uint32_t id,uint32_t bank,int logical){
 uint32_t base=logical?LSB:PSB;int32_t top=slot_max(m,logical?LSBMAX:PSBMAX);
 for(int32_t i=0;i<=top;i++){uint32_t slot=base+(uint32_t)i*8,r=rd(m,slot+4);if(rd(m,r)==id&&rd(m,r+4)==bank)return logical?r:slot;}
 return 0;
}
static void destroy_physical(M*m,uint32_t slot){
 if(playing(m,rd(m,slot)))stop_buffer(m,rd(m,slot));
 destroy_buffer(m,slot);wr(m,slot+4,0);
}
static uint32_t duplicate_buffer(M*m,uint32_t p){
 if(!rd(m,DEVICE)||!p)return 0;
 uint32_t out=new_buffer(m);if(!out)return 0;
 emit(m,MACOS_AUDIO_DUPLICATE,out,p,0,0);return out; /* SetCurrentPosition(0) follows; no command. */
}
static uint32_t create_logical(M*m,uint32_t p,uint32_t record){
 int32_t top=slot_max(m,LSBMAX);
 if(top==9)return 0;
 top++;wr(m,LSBMAX,(uint32_t)top);
 uint32_t out=duplicate_buffer(m,p),slot=LSB+(uint32_t)top*8;wr(m,slot,out);
 if(!out)return 0;
 wr(m,slot+4,record);return slot;
}
static void clear_dictator(M*m){
 if(!rd(m,DACTIVE))return;
 wr(m,DACTIVE,0);uint32_t slot=rd(m,DSLOT);
 stop_buffer(m,rd(m,slot));destroy_buffer(m,slot);wr(m,rd(m,DREF),NONE);
}
static int bank_index(uint32_t bank){for(int b=0;b<3;b++)if(BANKS[b]==bank)return b;return -1;}
/* HndSound_LoadDataInfo 0x48a0f4 with FindIdInBanque/GetDataHeader. */
static int load_info(M*m,uint32_t record,uint32_t id,uint32_t bank,uint32_t flags,uint32_t volume){
 uint32_t count=rw(m,bank+4),header=rd(m,bank+12),entry=NONE;
 for(uint32_t i=0;i<count;i++)if(rd(m,header+12*i)==id){entry=12*i;break;}
 wr(m,record+0x18,entry==NONE?0:header+entry);
 if(entry==NONE)return 0;
 wr(m,record,id);wr(m,record+4,bank);
 uint32_t counter=rd(m,VARIANT)+1;wr(m,VARIANT,counter);
 int32_t variants=(int32_t)rw(m,header+entry+6);uint32_t offset=rd(m,header+entry+8);
 uint32_t data=variants-1>0?offset+((((uint32_t)variants-1)&counter)<<4):offset;
 wr(m,record+0x1c,header+data);wr(m,record+0xc,flags);
 wr(m,record+0x10,rw(m,header+entry+4)==0x100?0x100:volume);wr(m,record+0x14,0);return 1;
}
static void destroy_only_sound(M*m,uint32_t r){
 uint32_t slot=rd(m,r+32);
 if(!slot)return;
 if(!stopped(m,rd(m,slot)))stop_buffer(m,rd(m,slot));
 if(rd(m,r+36)==1){destroy_logical(m,slot);wr(m,r+32,0);return;}
 destroy_physical(m,slot);
 uint32_t promoted=find_loaded(m,rd(m,r),rd(m,r+4),1);
 if(!promoted){replace_slot(m,slot,0);return;}
 uint32_t logical=rd(m,promoted+32);
 wr(m,slot,rd(m,logical));wr(m,slot+4,promoted);wr(m,logical,0);wr(m,logical+4,0);
 wr(m,promoted+32,slot);replace_slot(m,logical,1);wr(m,promoted+36,0);
}
static int stop_handle(M*m,uint32_t reference){
 uint32_t index=rd(m,reference);
 if(index==NONE)return 0;
 uint32_t r=address(m,index);
 if(rb(m,r+8)==1)destroy_only_sound(m,r);
 if(rd(m,rd(m,r+40))!=NONE&&rb(m,r+8)){
  if(rb(m,r+8)==2)compact(m,r,1);
  else{wr(m,rd(m,r+40),NONE);wb(m,r+8,0);wr(m,r+32,0);}
 }
 return 1;
}
/* PlaySnd_StopAllSound 0x48aac5 -> 0x48a4a8 over the 50 records 0x52137c+44*i:
 * a playing record (state 1) loses its sound (0x489e35); then, while its
 * caller reference is still held and the state is still set, a waiting record
 * (state 2) is compacted with the reference cleared (0x48af1b, EDX=1) and any
 * other one gets reference NONE, state 0 and +0x20 = 0 (stop_handle's tail). */
static void stop_all(M*m){
 for(uint32_t i=0;i<50;i++){
  uint32_t r=TABLE+i*STRIDE;
  if(!rb(m,r+8))continue;
  if(rb(m,r+8)==1)destroy_only_sound(m,r);
  if(rd(m,rd(m,r+40))!=NONE&&rb(m,r+8)){
   if(rb(m,r+8)==2)compact(m,r,1);
   else{wr(m,rd(m,r+40),NONE);wb(m,r+8,0);wr(m,r+32,0);}
  }
 }
}
static int set_volume(M*m,uint32_t reference,uint32_t volume){
 uint32_t index=rd(m,reference);
 if(index==NONE)return 0;
 uint32_t r=address(m,index);
 if(!rb(m,r+8))return 1;
 wr(m,r+16,volume);
 if(rw(m,rd(m,r+24)+4)==256)return 0;
 if(!volume)stop_handle(m,reference);
 else if(rb(m,r+8)==1){wr(m,VOLDIRTY,1);wr(m,r+20,1);}
 return 1;
}
static int insert(M*m,uint32_t id,uint32_t bank,uint32_t reference,uint32_t flags,uint32_t volume){
 uint32_t index=rd(m,reference);
 if(index!=NONE){
  uint32_t r=address(m,index);
  if(volume==0&&rw(m,rd(m,r+24)+4)!=256){stop_handle(m,reference);return 1;}
  if(rd(m,r)==id&&rd(m,r+4)==bank){set_volume(m,reference,volume);return 1;}
  if(rb(m,r+8)==1){
   stop_handle(m,reference);index=reserve(m);wr(m,reference,index);
   if(index==NONE)return 0;
   wr(m,REQUEST,1);
  }
  r=address(m,rd(m,reference));
  if(!load_info(m,r,id,bank,flags,volume))m->stats.absent++;
  wr(m,r+40,reference);wb(m,r+8,2);return 1;
 }
 index=reserve(m);
 if(index==NONE)return 0;
 uint32_t r=address(m,index);
 if(!load_info(m,r,id,bank,flags,volume)){m->stats.absent++;give_back(m);return 0;}
 wr(m,reference,index);wr(m,r+40,reference);wb(m,r+8,2);wr(m,r+20,0);wr(m,REQUEST,1);
 return 1;
}
static int32_t volume_word(M*m,uint32_t r){
 uint32_t multiplier=rb(m,rd(m,r+28)+13);if(!multiplier)multiplier=256;
 if(rw(m,rd(m,r+24)+4)==256)wr(m,r+16,256);
 int32_t lookup=i32(rd(m,r+16)*multiplier*rd(m,GAMEVOL))>>20;
 if(lookup<0||lookup>16)m->stats.domain_faults++;
 return i32(rd(m,GLB+(uint32_t)lookup*4));
}
static int start_buffer(M*m,uint32_t r){
 uint32_t p=rd(m,rd(m,r+32));int32_t volume=volume_word(m,r);
 if(!p)return 0;
 if(playing(m,p))return 1;
 Buffer*b=buffer(m,p);if(!b){m->stats.domain_faults++;return 0;}
 uint32_t loop=rd(m,r+12)==1;
 emit(m,MACOS_AUDIO_PLAY,p,0,volume,loop);b->status=1|(loop?4:0);return 1;
}
/* PSB_CreateMySurfaceWithSound through Stage-1 criar_superficie_som: formats 0
 * and 4 create a buffer filled with the decoded sample, other formats none. */
static uint32_t create_sound(M*m,uint32_t r){
 int b=bank_index(rd(m,r+4));uint32_t header=rd(m,r+28);
 if(b<0||header<HDR+(uint32_t)b*HDR_STRIDE){m->stats.domain_faults++;return 0;}
 uint32_t format=rb(m,header+12);
 if(format!=0&&format!=4)return 0;
 uint32_t id=new_buffer(m);if(!id)return 0;
 emit(m,MACOS_AUDIO_CREATE,id,((uint32_t)b<<24)|(header-HDR-(uint32_t)b*HDR_STRIDE),0,0);return id;
}
static uint32_t create_physical(M*m,uint32_t r){
 int32_t top=slot_max(m,PSBMAX);
 if(top==9)return 0;
 top++;wr(m,PSBMAX,(uint32_t)top);
 uint32_t p=create_sound(m,r),slot=PSB+(uint32_t)top*8;wr(m,slot,p);
 if(!p)return 0;
 wr(m,slot+4,r);return slot;
}
static int transfer_to_playing(M*m,uint32_t index){
 uint32_t destination=NONE;
 for(uint32_t i=0;i<20;i++)if(rb(m,address(m,i)+8)==0){destination=i;break;}
 if(destination==NONE)return 0;
 uint32_t source=address(m,index),target=address(m,destination);
 copy44(m,target,source);wr(m,rd(m,target+40),destination);
 wr(m,rd(m,target+32)+4,target);wb(m,target+8,1);
 return compact(m,source,0);
}
static void clear_list(M*m){
 const uint32_t bases[2]={LSB,PSB};
 for(unsigned k=0;k<2;k++)for(uint32_t i=0;i<10;i++){
  uint32_t slot=bases[k]+i*8,p=rd(m,slot);
  if(!p)continue;
  uint32_t r=rd(m,slot+4);
  if(rb(m,r+8)==1&&stopped(m,p))stop_handle(m,rd(m,r+40));
 }
 if(rd(m,DACTIVE)&&stopped(m,rd(m,rd(m,DSLOT2))))clear_dictator(m);
}
static uint32_t manage(M*m){
 clear_list(m);
 if(rd(m,VOLDIRTY)||rd(m,DIRTY)){
  for(uint32_t i=0;i<20;i++){
   uint32_t r=address(m,i);
   if(!rb(m,r+8)||rd(m,rd(m,r+40))==NONE)continue;
   uint32_t slot=rd(m,r+32);
   if(!slot||!rd(m,slot))continue;
   if(rd(m,r+20)!=1&&!rd(m,DIRTY))continue;
   int32_t value=volume_word(m,r);emit(m,MACOS_AUDIO_VOLUME,rd(m,slot),0,value,0);wr(m,r+20,0);
  }
  if(rd(m,DACTIVE)&&rd(m,DIRTY)){int32_t value=volume_word(m,DREC);emit(m,MACOS_AUDIO_VOLUME,rd(m,rd(m,DSLOT)),0,value,0);}
  wr(m,VOLDIRTY,0);
 }
 wr(m,DIRTY,0);
 uint32_t result=1,top=maximum(m);
 for(uint32_t index=top;index>19;index--){
  uint32_t r=address(m,index),loaded=find_loaded(m,rd(m,r),rd(m,r+4),0);
  if(rw(m,rd(m,r+24)+4)==256){
   if(rd(m,DACTIVE))continue;
   const uint32_t fields[8][2]={{12,0},{24,rd(m,r+24)},{4,rd(m,r+4)},{16,256},{40,rd(m,r+40)},{0,rd(m,r)},{28,rd(m,r+28)},{20,1}};
   for(unsigned f=0;f<8;f++)wr(m,DREC+fields[f][0],fields[f][1]);
   wr(m,rd(m,DSLOT),create_sound(m,DREC));
   if(!start_buffer(m,DREC))result=0;
   destroy_waiting(m,index);wr(m,DACTIVE,1);
   continue;
  }
  uint32_t slot;
  if(loaded){
   if(slot_max(m,LSBMAX)>=9){if(slot_max(m,PSBMAX)>=9)break;continue;}
   slot=create_logical(m,rd(m,loaded),r);wr(m,r+32,slot);
   if(!slot){result=0;continue;}
   wr(m,r+36,1);
  }else{
   if(slot_max(m,PSBMAX)>=9)continue;
   slot=create_physical(m,r);wr(m,r+32,slot);
   if(!slot){result=0;continue;}
   wr(m,r+36,0);
  }
  if(!start_buffer(m,r))result=0;
  transfer_to_playing(m,index);wr(m,REQUEST,1);
 }
 return result;
}

uint32_t macos_audio_abi_version(void){return 1;}
int32_t macos_audio_volume_table(uint32_t index){return index<17?ORIGINAL_GLOBALS[index]:0;}
MacOSAudioMapper *macos_audio_create(void){
 M*m=calloc(1,sizeof(M));if(!m)return NULL;
 for(unsigned i=0;i<22;i++){uint32_t v=(uint32_t)ORIGINAL_GLOBALS[i];memcpy(m->glb+4*i,&v,4);}
 m->next_buffer=1;
 /* Library state after PlaySnd_InitSoundLib: device pointer, bank records
  * (+0 open, +4 count, +0xc header) and HandleSnd_InitDictator's effects. */
 wr(m,DEVICE,DEVICE_POINTER);
 wr(m,DSLOT2,DICT);wr(m,DSLOT,DICT);wr(m,DICT+4,DREC);wr(m,DACTIVE,0);wr(m,DFLAG,1);wr(m,ALLOCS,rd(m,ALLOCS)+1);
 return m;
}
void macos_audio_free(MacOSAudioMapper*m){free(m);}
int32_t macos_audio_load_bank(MacOSAudioMapper*m,uint32_t selector,const uint8_t*hed,uint32_t length){
 int b=-1;for(int i=0;i<3;i++)if(SELECTORS[i]==selector)b=i;
 if(!m||b<0||!hed||length<16||memcmp(hed,"ABCD",4))return 1;
 uint32_t body=le(hed+4),count=le(hed+12);
 if(body!=length-16||body>HDR_STRIDE||count>0xffff||(uint64_t)count*12>body)return 1;
 memcpy(m->hdr[b],hed+16,body);m->hdr_len[b]=body;
 uint32_t bank=BANKS[b];wr(m,bank,1);wr(m,bank+4,count);wr(m,bank+12,HDR+(uint32_t)b*HDR_STRIDE);
 return 0;
}
uint32_t macos_audio_request(MacOSAudioMapper*m,const MacOSSoundEvent*e){
 if(!m||!e)return 0;
 m->frame=e->frame;m->phase=e->phase;m->stats.requests++;
 if(e->kind==0){m->stats.manages++;return manage(m);}
 if(e->kind==2){m->stats.stops++;return (uint32_t)stop_handle(m,e->eax);}
 if(e->kind==3){m->stats.stop_alls++;stop_all(m);return 1;}
 if(e->kind!=1){m->stats.domain_faults++;return 0;}
 m->stats.posts++;
 for(int b=0;b<3;b++)if(SELECTORS[b]==e->eax)return (uint32_t)insert(m,e->edx,BANKS[b],e->ebx,e->ecx&255,e->volume);
 m->stats.unsupported_selector++;return e->edx; /* Original EAX is EDX. */
}
void macos_audio_device_stopped(MacOSAudioMapper*m,uint32_t id){Buffer*b=m?buffer(m,id):NULL;if(b)b->status=0;}
uint32_t macos_audio_commands(MacOSAudioMapper*m,MacOSAudioCommand*out,uint32_t capacity){
 if(!m||!out)return 0;uint32_t n=m->command_count<capacity?m->command_count:capacity;
 memcpy(out,m->commands,n*sizeof(*out));memmove(m->commands,m->commands+n,(m->command_count-n)*sizeof(*out));m->command_count-=n;return n;
}
int32_t macos_audio_sample(MacOSAudioMapper*m,uint32_t key,uint32_t*bank,uint32_t*index,uint32_t*variant){
 uint32_t b=key>>24,offset=key&0xffffff;
 if(!m||b>2||!m->hdr_len[b])return 1;
 uint32_t count=rw(m,BANKS[b]+4);const uint8_t*h=m->hdr[b];
 for(uint32_t i=0;i<count;i++){
  uint32_t first=le(h+12*i+8),n=h[12*i+6]|((uint32_t)h[12*i+7]<<8);
  if(offset>=first&&offset<first+16*n&&(offset-first)%16==0){if(bank)*bank=b;if(index)*index=i;if(variant)*variant=(offset-first)/16;return 0;}
 }
 return 1;
}
uint32_t macos_audio_reference(MacOSAudioMapper*m,uint32_t address){return m?rd(m,address):NONE;}
void macos_audio_stats(MacOSAudioMapper*m,MacOSAudioStats*out){if(m&&out)*out=m->stats;}
