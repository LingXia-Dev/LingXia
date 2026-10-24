package com.lingxia.test.receiver;
import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.widget.TextView;
import java.io.*;
import java.util.ArrayList;
import java.security.MessageDigest;
import org.json.*;
public final class Receiver extends Activity {
 @Override public void onCreate(Bundle state){super.onCreate(state);TextView view=new TextView(this);view.setPadding(30,100,30,30);view.setTextSize(22);setContentView(view);
  try{
   Intent intent=getIntent();JSONObject record=new JSONObject();record.put("action",intent.getAction());record.put("mime",intent.getType());record.put("text",String.valueOf(intent.getCharSequenceExtra(Intent.EXTRA_TEXT)));
   ArrayList<Uri> uris=intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM);if(uris==null){uris=new ArrayList<>();Uri single=intent.getParcelableExtra(Intent.EXTRA_STREAM);if(single!=null)uris.add(single);}
   JSONArray files=new JSONArray();for(Uri uri:uris){MessageDigest digest=MessageDigest.getInstance("SHA-256");long size=0;try(InputStream input=getContentResolver().openInputStream(uri)){byte[] buffer=new byte[8192];int count;while((count=input.read(buffer))!=-1){size+=count;digest.update(buffer,0,count);}}
    StringBuilder hash=new StringBuilder();for(byte b:digest.digest())hash.append(String.format("%02x",b&255));JSONObject file=new JSONObject().put("uri",uri.toString()).put("size",size).put("sha256",hash.toString());
    if(intent.getType()!=null&&intent.getType().startsWith("image/")){android.graphics.BitmapFactory.Options options=new android.graphics.BitmapFactory.Options();options.inJustDecodeBounds=true;try(InputStream input=getContentResolver().openInputStream(uri)){android.graphics.BitmapFactory.decodeStream(input,null,options);}file.put("width",options.outWidth).put("height",options.outHeight);}
    files.put(file);}
   record.put("files",files);try(FileOutputStream output=openFileOutput("received.jsonl",MODE_APPEND)){output.write((record.toString()+"\n").getBytes("UTF-8"));}
   view.setText("Received locally");
  }catch(Exception error){view.setText("Receiver error: "+error);}
 }
}
