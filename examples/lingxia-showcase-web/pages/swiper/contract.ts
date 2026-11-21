import type { PageContract } from '@lingxia/types/page';

export interface SwiperItem {
  id: string;
  type: 'image' | 'video';
  src: string;
}

export type ObjectFit = 'cover' | 'contain' | 'fill';
export type Animation = 'slide' | 'none';
export type Direction = 'horizontal' | 'vertical';

export type SwiperPage = PageContract<
  {
    items: SwiperItem[];
    index: number;
    autoplay: boolean;
    loop: boolean;
    dots: boolean;
    objectFit: ObjectFit;
    animation: Animation;
    direction: Direction;
    peek: number;
    eventLog: string;
    busy: boolean;
  },
  {
    pickImages(): Promise<void>;
    pickVideos(): Promise<void>;
    removeCurrent(): Promise<void>;
    clearAll(): Promise<void>;
    toggleAutoplay(): Promise<void>;
    toggleLoop(): Promise<void>;
    toggleDots(): Promise<void>;
    setObjectFit(params: { fit: ObjectFit }): Promise<void>;
    setAnimation(params: { animation: Animation }): Promise<void>;
    setDirection(params: { direction: Direction }): Promise<void>;
    setPeek(params: { value: number }): Promise<void>;
    onSwiperChange(detail: { index: number }): Promise<void>;
    onSwiperTransitionEnd(detail: { index: number }): Promise<void>;
    onSwiperEndReached(): Promise<void>;
    onSwiperTap(detail: { index: number }): Promise<void>;
    onSwiperVideoEnded(detail: { index: number }): Promise<void>;
    onSwiperError(detail: { code?: string }): Promise<void>;
  }
>;
