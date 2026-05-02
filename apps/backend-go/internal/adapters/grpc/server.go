package cmsv1

import (
	"context"
	"fmt"
	"net"
	"strings"

	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
	"github.com/velopulent/cms/apps/backend-go/internal/domain"
	"github.com/velopulent/cms/apps/backend-go/internal/ports"
)

type Server struct {
	UnimplementedCollectionServiceServer
	UnimplementedEntryServiceServer
	UnimplementedSingletonServiceServer
	UnimplementedFileServiceServer
	UnimplementedSiteServiceServer
	UnimplementedMembershipServiceServer
	UnimplementedTokenServiceServer
	s *application.Services
}

func NewServer(s *application.Services) *Server { return &Server{s: s} }

func Start(ctx context.Context, addr string, s *application.Services) error {
	l, err := net.Listen("tcp", addr)
	if err != nil { return err }
	g := grpc.NewServer()
	srv := NewServer(s)
	RegisterCollectionServiceServer(g, srv); RegisterEntryServiceServer(g, srv); RegisterSingletonServiceServer(g, srv)
	RegisterFileServiceServer(g, srv); RegisterSiteServiceServer(g, srv); RegisterMembershipServiceServer(g, srv); RegisterTokenServiceServer(g, srv)
	go func(){ <-ctx.Done(); g.GracefulStop() }()
	return g.Serve(l)
}

func (s *Server) principal(ctx context.Context) (ports.Principal, error) {
	md, _ := metadata.FromIncomingContext(ctx)
	auth := ""
	if vals := md.Get("authorization"); len(vals)>0 { auth = vals[0] }
	token := strings.TrimSpace(strings.TrimPrefix(auth, "Bearer "))
	if strings.HasPrefix(token, "cms_") { return s.s.Tokens.Verify(ctx, token) }
	if token == "" { return ports.Principal{}, status.Error(codes.Unauthenticated, "Missing authentication") }
	claims, err := s.s.Auth.VerifyToken(token); if err != nil { return ports.Principal{}, status.Error(codes.Unauthenticated, "Invalid or expired token") }
	return ports.Principal{Kind:ports.PrincipalUser, UserID:claims.Sub}, nil
}
func (s *Server) site(ctx context.Context, scope string) (string,error) { p,err:=s.principal(ctx); if err!=nil{return "",err}; if p.Kind==ports.PrincipalSite { if !p.Scopes[scope]{return "",status.Error(codes.PermissionDenied,"insufficient scope")}; return p.SiteID,nil }; md,_:=metadata.FromIncomingContext(ctx); vals:=md.Get(application.HeaderSiteID); if len(vals)==0{return "",status.Error(codes.PermissionDenied,"Missing site context")}; return vals[0],nil }

func pbCollection(c *domain.Collection)*Collection{ if c==nil{return nil}; return &Collection{Id:c.ID, SiteId:c.SiteID, Name:c.Name, Slug:c.Slug, Definition:c.Definition, IsSingleton:c.IsSingleton, SingletonData:c.SingletonData, CreatedAt:c.CreatedAt, UpdatedAt:c.UpdatedAt} }
func pbEntry(e *domain.Entry)*Entry{ if e==nil{return nil}; return &Entry{Id:e.ID, SiteId:e.SiteID, CollectionId:e.CollectionID, Data:e.Data, Slug:e.Slug, Status:e.Status, CreatedAt:e.CreatedAt, UpdatedAt:e.UpdatedAt, PublishedAt:e.PublishedAt} }
func pbSite(v *domain.Site)*Site{ if v==nil{return nil}; return &Site{Id:v.ID, Name:v.Name, StorageProvider:v.StorageProvider, CreatedBy:v.CreatedBy, CreatedAt:v.CreatedAt, UpdatedAt:v.UpdatedAt} }
func pbMember(v *domain.SiteMember)*SiteMember{ if v==nil{return nil}; return &SiteMember{Id:v.ID, SiteId:v.SiteID, UserId:v.UserID, Username:v.Username, Email:v.Email, Role:v.Role, CreatedAt:v.CreatedAt} }
func pbFile(v *domain.File)*File{ if v==nil{return nil}; var w,h *int32; if v.Width!=nil{x:=int32(*v.Width);w=&x}; if v.Height!=nil{x:=int32(*v.Height);h=&x}; return &File{Id:v.ID, SiteId:v.SiteID, Filename:v.Filename, OriginalName:v.OriginalName, MimeType:v.MimeType, Size:v.Size, StorageProvider:v.StorageProvider, StorageKey:v.StorageKey, ThumbnailKey:v.ThumbnailKey, Width:w, Height:h, DeletedAt:v.DeletedAt, CreatedBy:v.CreatedBy, CreatedAt:v.CreatedAt} }
func pbToken(t domain.AccessToken)*AccessToken{ scopes:=[]string{}; for _,s:=range strings.Split(t.Scopes,","){if strings.TrimSpace(s)!=""{scopes=append(scopes,strings.TrimSpace(s))}}; return &AccessToken{Id:t.ID, Kind:t.Kind, SiteId:t.SiteID, Name:t.Name, TokenPrefix:t.TokenPrefix, Scopes:scopes, CreatedByUserId:t.CreatedByUserID, LastUsedAt:t.LastUsedAt, CreatedAt:t.CreatedAt, ExpiresAt:t.ExpiresAt, RevokedAt:t.RevokedAt} }

func (s *Server) ListCollections(ctx context.Context,*ListCollectionsRequest)(*ListCollectionsResponse,error){ site,err:=s.site(ctx,application.ScopeSchemaRead); if err!=nil{return nil,err}; v,err:=s.s.Collections.List(ctx,site); if err!=nil{return nil,status.Error(codes.Internal,application.Message(err))}; out:=&ListCollectionsResponse{}; for _,c:=range v{out.Collections=append(out.Collections,pbCollection(&c))}; return out,nil }
func (s *Server) GetCollection(ctx context.Context,r *GetCollectionRequest)(*Collection,error){ site,err:=s.site(ctx,application.ScopeSchemaRead); if err!=nil{return nil,err}; v,err:=s.s.Collections.Get(ctx,site,r.Slug); if err!=nil||v==nil{return nil,status.Error(codes.NotFound,"Collection not found")}; return pbCollection(v),nil }
func (s *Server) CreateCollection(ctx context.Context,r *CreateCollectionRequest)(*Collection,error){ site,err:=s.site(ctx,application.ScopeSchemaWrite); if err!=nil{return nil,err}; v,err:=s.s.Collections.Create(ctx,site,r.Name,r.Slug,r.Definition,r.IsSingleton); if err!=nil{return nil,status.Error(codes.Internal,application.Message(err))}; return pbCollection(v),nil }
func (s *Server) UpdateCollection(ctx context.Context,r *UpdateCollectionRequest)(*Collection,error){ site,err:=s.site(ctx,application.ScopeSchemaWrite); if err!=nil{return nil,err}; var def any; if r.Definition!=nil{def=*r.Definition}; v,err:=s.s.Collections.Update(ctx,site,r.Id,r.Name,r.Slug,def); if err!=nil{return nil,status.Error(codes.Internal,application.Message(err))}; return pbCollection(v),nil }
func (s *Server) DeleteCollection(ctx context.Context,r *DeleteCollectionRequest)(*DeleteResponse,error){ site,err:=s.site(ctx,application.ScopeSchemaWrite); if err!=nil{return nil,err}; _,err=s.s.Collections.Delete(ctx,site,r.Slug); return &DeleteResponse{Success:err==nil, Message:"Collection deleted"},err }

func (s *Server) ListEntries(ctx context.Context,r *ListEntriesRequest)(*ListEntriesResponse,error){ site,err:=s.site(ctx,application.ScopeContentRead); if err!=nil{return nil,err}; p:=ports.ListEntriesParams{SiteID:site,Page:r.Page,PerPage:r.PerPage}; if r.CollectionId!=nil{p.CollectionID=*r.CollectionId}; if r.Status!=nil{p.Status=*r.Status}; if r.Search!=nil{p.Search=*r.Search}; v,err:=s.s.Entries.List(ctx,p); if err!=nil{return nil,status.Error(codes.Internal,application.Message(err))}; out:=&ListEntriesResponse{Total:v.Total,Page:v.Page,PerPage:v.PerPage}; for _,e:=range v.Items{out.Items=append(out.Items,pbEntry(&e))}; return out,nil }
func (s *Server) GetEntry(ctx context.Context,r *GetEntryRequest)(*Entry,error){ site,err:=s.site(ctx,application.ScopeContentRead); if err!=nil{return nil,err}; v,err:=s.s.Entries.Get(ctx,r.Id,site,false); if err!=nil||v==nil{return nil,status.Error(codes.NotFound,"Entry not found")}; return pbEntry(v),nil }
func (s *Server) CreateEntry(ctx context.Context,r *CreateEntryRequest)(*Entry,error){ site,err:=s.site(ctx,application.ScopeContentWrite); if err!=nil{return nil,err}; v,err:=s.s.Entries.Create(ctx,site,r.CollectionId,r.Data,r.Slug,nil); if err!=nil{return nil,status.Error(codes.Internal,application.Message(err))}; return pbEntry(v),nil }
func (s *Server) UpdateEntry(ctx context.Context,r *UpdateEntryRequest)(*Entry,error){ site,err:=s.site(ctx,application.ScopeContentWrite); if err!=nil{return nil,err}; var data any; if r.Data!=nil{data=*r.Data}; v,err:=s.s.Entries.Update(ctx,r.Id,site,data,r.Slug,r.Status,nil,r.ChangeSummary); if err!=nil{return nil,status.Error(codes.Internal,application.Message(err))}; return pbEntry(v),nil }
func (s *Server) DeleteEntry(ctx context.Context,r *DeleteEntryRequest)(*DeleteResponse,error){ site,err:=s.site(ctx,application.ScopeContentWrite); if err!=nil{return nil,err}; _,err=s.s.Entries.Delete(ctx,r.Id,site); return &DeleteResponse{Success:err==nil,Message:"Entry deleted"},err }
func (s *Server) PublishEntry(ctx context.Context,r *PublishEntryRequest)(*Entry,error){ site,err:=s.site(ctx,application.ScopeContentWrite); if err!=nil{return nil,err}; v,err:=s.s.Entries.Publish(ctx,r.Id,site); if err!=nil{return nil,status.Error(codes.NotFound,"Entry not found")}; return pbEntry(v),nil }
func (s *Server) UnpublishEntry(ctx context.Context,r *UnpublishEntryRequest)(*Entry,error){ site,err:=s.site(ctx,application.ScopeContentWrite); if err!=nil{return nil,err}; v,err:=s.s.Entries.Unpublish(ctx,r.Id,site); if err!=nil{return nil,status.Error(codes.NotFound,"Entry not found")}; return pbEntry(v),nil }

func (s *Server) ListEntryRevisions(ctx context.Context,r *ListEntryRevisionsRequest)(*ListEntryRevisionsResponse,error){ site,err:=s.site(ctx,application.ScopeContentRead); if err!=nil{return nil,err}; v,err:=s.s.Entries.ListRevisions(ctx,r.EntryId,site,r.Page,r.PerPage); if err!=nil{return nil,status.Error(codes.Internal,application.Message(err))}; out:=&ListEntryRevisionsResponse{Total:v.Total,Page:v.Page,PerPage:v.PerPage}; for _,x:=range v.Items{out.Items=append(out.Items,&EntryRevision{Id:x.ID,EntryId:x.EntryID,RevisionNumber:x.RevisionNumber,Data:x.Data,CreatedBy:x.CreatedBy,CreatedAt:x.CreatedAt,ChangeSummary:x.ChangeSummary})}; return out,nil}
func (s *Server) GetEntryRevision(ctx context.Context,r *GetEntryRevisionRequest)(*EntryRevision,error){ site,err:=s.site(ctx,application.ScopeContentRead); if err!=nil{return nil,err}; x,err:=s.s.Entries.GetRevision(ctx,r.EntryId,site,r.RevisionNumber); if err!=nil||x==nil{return nil,status.Error(codes.NotFound,"Revision not found")}; return &EntryRevision{Id:x.ID,EntryId:x.EntryID,RevisionNumber:x.RevisionNumber,Data:x.Data,CreatedBy:x.CreatedBy,CreatedAt:x.CreatedAt,ChangeSummary:x.ChangeSummary},nil}
func (s *Server) RestoreEntryRevision(ctx context.Context,r *RestoreEntryRevisionRequest)(*Entry,error){ site,err:=s.site(ctx,application.ScopeContentWrite); if err!=nil{return nil,err}; v,err:=s.s.Entries.RestoreRevision(ctx,r.EntryId,site,r.RevisionNumber,nil); if err!=nil{return nil,status.Error(codes.NotFound,"Revision not found")}; return pbEntry(v),nil}

func (s *Server) GetSingleton(ctx context.Context,r *GetSingletonRequest)(*Singleton,error){ site,err:=s.site(ctx,application.ScopeSchemaRead); if err!=nil{return nil,err}; v,err:=s.s.Collections.GetSingleton(ctx,site,r.Slug); if err!=nil{return nil,status.Error(codes.NotFound,"Singleton not found")}; data:=""; if v.Data!=nil{data=fmt.Sprint(v.Data)}; return &Singleton{Id:v.ID,SiteId:v.SiteID,Name:v.Name,Slug:v.Slug,Definition:fmt.Sprint(v.Definition),Data:&data,CreatedAt:v.CreatedAt,UpdatedAt:v.UpdatedAt},nil}
func (s *Server) UpdateSingleton(ctx context.Context,r *UpdateSingletonRequest)(*Singleton,error){ site,err:=s.site(ctx,application.ScopeSchemaWrite); if err!=nil{return nil,err}; v,err:=s.s.Collections.UpdateSingleton(ctx,site,r.Slug,r.Data); if err!=nil{return nil,status.Error(codes.NotFound,"Singleton not found")}; data:=r.Data; return &Singleton{Id:v.ID,SiteId:v.SiteID,Name:v.Name,Slug:v.Slug,Definition:fmt.Sprint(v.Definition),Data:&data,CreatedAt:v.CreatedAt,UpdatedAt:v.UpdatedAt},nil}

func (s *Server) ListFiles(ctx context.Context,r *ListFilesRequest)(*ListFilesResponse,error){ site,err:=s.site(ctx,application.ScopeAssetsRead); if err!=nil{return nil,err}; p:=ports.ListFilesParams{SiteID:site,Page:r.Page,PerPage:r.PerPage}; if r.Search!=nil{p.Search=*r.Search}; if r.FileType!=nil{p.FileType=*r.FileType}; v,err:=s.s.Files.List(ctx,p); if err!=nil{return nil,status.Error(codes.Internal,application.Message(err))}; out:=&ListFilesResponse{Total:v.Total,Page:v.Page,PerPage:v.PerPage}; for _,f:=range v.Items{out.Files=append(out.Files,pbFile(&f))}; return out,nil}
func (s *Server) GetFile(ctx context.Context,r *GetFileRequest)(*File,error){ site,err:=s.site(ctx,application.ScopeAssetsRead); if err!=nil{return nil,err}; v,err:=s.s.Files.Get(ctx,r.Id,site); if err!=nil||v==nil{return nil,status.Error(codes.NotFound,"File not found")}; return pbFile(v),nil}
func (s *Server) DeleteFile(ctx context.Context,r *DeleteFileRequest)(*DeleteResponse,error){ site,err:=s.site(ctx,application.ScopeAssetsWrite); if err!=nil{return nil,err}; _,err=s.s.Files.SoftDelete(ctx,r.Id,site); return &DeleteResponse{Success:err==nil,Message:"File deleted"},err}
func (s *Server) RestoreFile(ctx context.Context,r *RestoreFileRequest)(*File,error){ site,err:=s.site(ctx,application.ScopeAssetsWrite); if err!=nil{return nil,err}; _,err=s.s.Files.Restore(ctx,r.Id,site); if err!=nil{return nil,err}; v,_:=s.s.Files.Get(ctx,r.Id,site); return pbFile(v),nil}
func (s *Server) BatchDeleteFiles(ctx context.Context,r *BatchDeleteFilesRequest)(*BatchOperationResponse,error){ site,err:=s.site(ctx,application.ScopeAssetsWrite); if err!=nil{return nil,err}; n,err:=s.s.Files.BatchSoftDelete(ctx,site,r.Ids); return &BatchOperationResponse{Affected:n},err}
func (s *Server) BatchRestoreFiles(ctx context.Context,r *BatchRestoreFilesRequest)(*BatchOperationResponse,error){ site,err:=s.site(ctx,application.ScopeAssetsWrite); if err!=nil{return nil,err}; n,err:=s.s.Files.BatchRestore(ctx,site,r.Ids); return &BatchOperationResponse{Affected:n},err}

func (s *Server) ListSites(ctx context.Context,*ListSitesRequest)(*ListSitesResponse,error){ v,err:=s.s.Sites.ListForPrincipal(ctx,ports.Principal{Kind:ports.PrincipalInstance}); if err!=nil{return nil,err}; out:=&ListSitesResponse{}; for _,m:=range v{out.Sites=append(out.Sites,&Site{Id:m["id"].(string),Name:m["name"].(string),StorageProvider:m["storage_provider"].(string),CreatedBy:m["created_by"].(string),CreatedAt:m["created_at"].(string),UpdatedAt:m["updated_at"].(string)})}; return out,nil}
func (s *Server) GetSite(ctx context.Context,r *GetSiteRequest)(*Site,error){ v,err:=s.s.Sites.Get(ctx,r.SiteId); if err!=nil||v==nil{return nil,status.Error(codes.NotFound,"Site not found")}; return pbSite(v),nil}
func (s *Server) CreateSite(ctx context.Context,r *CreateSiteRequest)(*Site,error){ sp:="filesystem"; if r.StorageProvider!=nil{sp=*r.StorageProvider}; v,err:=s.s.Sites.Create(ctx,r.Name,sp,"system"); if err!=nil{return nil,err}; return pbSite(v),nil}
func (s *Server) UpdateSite(ctx context.Context,r *UpdateSiteRequest)(*Site,error){ v,err:=s.s.Sites.Update(ctx,r.SiteId,r.Name); if err!=nil{return nil,err}; return pbSite(v),nil}
func (s *Server) DeleteSite(ctx context.Context,r *DeleteSiteRequest)(*DeleteResponse,error){ _,err:=s.s.Sites.Delete(ctx,r.SiteId); return &DeleteResponse{Success:err==nil,Message:"Site deleted"},err}
func (s *Server) ListMembers(ctx context.Context,r *ListMembersRequest)(*ListMembersResponse,error){ v,err:=s.s.Sites.ListMembers(ctx,r.SiteId); if err!=nil{return nil,err}; out:=&ListMembersResponse{}; for _,m:=range v{out.Members=append(out.Members,pbMember(&m))}; return out,nil}
func (s *Server) InviteMember(ctx context.Context,r *InviteMemberRequest)(*SiteMember,error){ v,err:=s.s.Sites.InviteMember(ctx,r.SiteId,r.Username,r.Role); if err!=nil{return nil,err}; return pbMember(v),nil}
func (s *Server) UpdateMemberRole(ctx context.Context,r *UpdateMemberRoleRequest)(*SiteMember,error){ v,err:=s.s.Sites.UpdateMemberRole(ctx,r.SiteId,r.UserId,r.Role); if err!=nil{return nil,err}; return pbMember(v),nil}
func (s *Server) RemoveMember(ctx context.Context,r *RemoveMemberRequest)(*DeleteResponse,error){ _,err:=s.s.Sites.RemoveMember(ctx,r.SiteId,r.UserId,""); return &DeleteResponse{Success:err==nil,Message:"Member removed"},err}
func (s *Server) ListInstanceTokens(ctx context.Context,*ListInstanceTokensRequest)(*ListInstanceTokensResponse,error){ v,err:=s.s.Tokens.List(ctx,"instance",nil); if err!=nil{return nil,err}; out:=&ListInstanceTokensResponse{}; for _,t:=range v{out.Tokens=append(out.Tokens,pbToken(t))}; return out,nil}
func (s *Server) CreateInstanceToken(ctx context.Context,r *CreateInstanceTokenRequest)(*CreateTokenResponse,error){ v,err:=s.s.Tokens.Create(ctx,"instance",nil,r.Name,r.Scopes,nil); if err!=nil{return nil,err}; return &CreateTokenResponse{AccessToken:&AccessToken{Id:v.ID,Kind:v.Kind,SiteId:v.SiteID,Name:v.Name,TokenPrefix:v.TokenPrefix,Scopes:v.Scopes,CreatedAt:v.CreatedAt},Token:v.Token},nil}
func (s *Server) DeleteInstanceToken(ctx context.Context,r *DeleteInstanceTokenRequest)(*DeleteResponse,error){ _,err:=s.s.Tokens.Delete(ctx,r.TokenId,"instance",nil); return &DeleteResponse{Success:err==nil,Message:"Token deleted"},err}
func (s *Server) ListSiteTokens(ctx context.Context,r *ListSiteTokensRequest)(*ListSiteTokensResponse,error){ v,err:=s.s.Tokens.List(ctx,"site",&r.SiteId); if err!=nil{return nil,err}; out:=&ListSiteTokensResponse{}; for _,t:=range v{out.Tokens=append(out.Tokens,pbToken(t))}; return out,nil}
func (s *Server) CreateSiteToken(ctx context.Context,r *CreateSiteTokenRequest)(*CreateTokenResponse,error){ v,err:=s.s.Tokens.Create(ctx,"site",&r.SiteId,r.Name,r.Scopes,nil); if err!=nil{return nil,err}; return &CreateTokenResponse{AccessToken:&AccessToken{Id:v.ID,Kind:v.Kind,SiteId:v.SiteID,Name:v.Name,TokenPrefix:v.TokenPrefix,Scopes:v.Scopes,CreatedAt:v.CreatedAt},Token:v.Token},nil}
func (s *Server) DeleteSiteToken(ctx context.Context,r *DeleteSiteTokenRequest)(*DeleteResponse,error){ _,err:=s.s.Tokens.Delete(ctx,r.TokenId,"site",&r.SiteId); return &DeleteResponse{Success:err==nil,Message:"Token deleted"},err}
